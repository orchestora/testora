//! Local-only demo API. Not the production control plane: no auth, no
//! persistence, no tenant isolation. Never expose it beyond loopback.

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
    routing::get,
};
use testora_contracts::{
    ApiError, CreateEnvironmentRequest, Environment, EnvironmentStatus, HealthResponse,
};
use tokio::sync::RwLock;
use uuid::Uuid;

pub const SERVICE_NAME: &str = "testora-api";
pub const DEFAULT_BIND: &str = "127.0.0.1:8080";

type Store = Arc<RwLock<HashMap<Uuid, Environment>>>;
type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;

/// Router with a fresh in-memory store. Clones share the store.
pub fn app() -> Router {
    let store: Store = Arc::default();
    Router::new()
        .route("/health", get(health))
        .route(
            "/v1/environments",
            get(list_environments).post(create_environment),
        )
        .route(
            "/v1/environments/{id}",
            get(get_environment).delete(destroy_environment),
        )
        .fallback(|| async { error(StatusCode::NOT_FOUND, "route not found") })
        .with_state(store)
}

/// Parse a listen address and refuse anything but loopback, so the demo
/// can't be exposed to a network by accident.
pub fn parse_bind(value: &str) -> Result<SocketAddr, String> {
    let addr: SocketAddr = value
        .parse()
        .map_err(|_| format!("{value:?} is not a host:port address"))?;
    if !addr.ip().is_loopback() {
        return Err(format!(
            "{addr} is not a loopback address; the demo API only listens on 127.0.0.1 or ::1"
        ));
    }
    Ok(addr)
}

fn error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (
        status,
        Json(ApiError {
            error: message.into(),
        }),
    )
}

fn not_found() -> (StatusCode, Json<ApiError>) {
    error(StatusCode::NOT_FOUND, "environment not found")
}

fn environment_id(path: Result<Path<Uuid>, PathRejection>) -> ApiResult<Uuid> {
    path.map(|Path(id)| id)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "environment id must be a UUID"))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        service: SERVICE_NAME.into(),
    })
}

async fn list_environments(State(store): State<Store>) -> Json<Vec<Environment>> {
    let mut envs: Vec<Environment> = store.read().await.values().cloned().collect();
    envs.sort_by_key(|env| env.id);
    Json(envs)
}

async fn create_environment(
    State(store): State<Store>,
    body: Result<Json<CreateEnvironmentRequest>, JsonRejection>,
) -> ApiResult<(StatusCode, Json<Environment>)> {
    // Keep axum's status (400 bad JSON, 415 wrong content type, 422 wrong
    // shape) but always answer with the JSON error body.
    let Json(req) = body.map_err(|rejection| error(rejection.status(), rejection.body_text()))?;
    req.validate()
        .map_err(|e| error(StatusCode::BAD_REQUEST, e.to_string()))?;

    let env = Environment {
        id: Uuid::new_v4(),
        project: req.project,
        region: req.region,
        status: EnvironmentStatus::Requested,
    };
    store.write().await.insert(env.id, env.clone());
    Ok((StatusCode::ACCEPTED, Json(env)))
}

async fn get_environment(
    State(store): State<Store>,
    path: Result<Path<Uuid>, PathRejection>,
) -> ApiResult<Json<Environment>> {
    let id = environment_id(path)?;
    store
        .read()
        .await
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(not_found)
}

/// Idempotent: destroying twice returns the same result.
async fn destroy_environment(
    State(store): State<Store>,
    path: Result<Path<Uuid>, PathRejection>,
) -> ApiResult<(StatusCode, Json<Environment>)> {
    let id = environment_id(path)?;
    let mut guard = store.write().await;
    let env = guard.get_mut(&id).ok_or_else(not_found)?;
    env.status = EnvironmentStatus::DestroyRequested;
    Ok((StatusCode::ACCEPTED, Json(env.clone())))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use axum::{
        body::Body,
        http::{Method, Request, Response, header},
    };
    use http_body_util::BodyExt;
    use serde::de::DeserializeOwned;
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;

    async fn call(app: &Router, method: Method, uri: &str, body: Option<&str>) -> Response<Body> {
        let mut req = Request::builder().method(method).uri(uri);
        if body.is_some() {
            req = req.header(header::CONTENT_TYPE, "application/json");
        }
        let req = req
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_owned())))
            .unwrap();
        app.clone().oneshot(req).await.unwrap()
    }

    async fn read<T: DeserializeOwned>(res: Response<Body>) -> T {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|e| panic!("bad JSON body {:?}: {e}", String::from_utf8_lossy(&bytes)))
    }

    async fn expect_error(res: Response<Body>, status: StatusCode, needle: &str) {
        assert_eq!(res.status(), status);
        assert_eq!(
            res.headers()[header::CONTENT_TYPE],
            "application/json",
            "errors must be JSON"
        );
        let err: ApiError = read(res).await;
        assert!(
            err.error.contains(needle),
            "expected {needle:?} in error {:?}",
            err.error
        );
    }

    async fn create(app: &Router, project: &str, region: &str) -> Environment {
        let body = json!({ "project": project, "region": region }).to_string();
        let res = call(app, Method::POST, "/v1/environments", Some(&body)).await;
        assert_eq!(res.status(), StatusCode::ACCEPTED);
        read(res).await
    }

    #[tokio::test]
    async fn health_reports_ok() {
        let res = call(&app(), Method::GET, "/health", None).await;
        assert_eq!(res.status(), StatusCode::OK);
        let health: HealthResponse = read(res).await;
        assert_eq!(health.status, "ok");
        assert_eq!(health.service, SERVICE_NAME);
    }

    #[tokio::test]
    async fn create_returns_requested_environment() {
        let app = app();
        let env = create(&app, "demo", "us-east").await;
        assert_eq!(env.project, "demo");
        assert_eq!(env.region, "us-east");
        assert_eq!(env.status, EnvironmentStatus::Requested);

        let res = call(
            &app,
            Method::GET,
            &format!("/v1/environments/{}", env.id),
            None,
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(read::<Environment>(res).await, env);
    }

    #[tokio::test]
    async fn create_rejects_invalid_fields_without_storing() {
        let app = app();
        let long_project = "p".repeat(testora_contracts::PROJECT_MAX_LEN + 1);
        let cases = [
            ("", "us-east", "project"),
            ("   ", "us-east", "project"),
            ("demo app", "us-east", "project"),
            (long_project.as_str(), "us-east", "project"),
            ("demo", "", "region"),
            ("demo", "US_EAST", "region"),
            ("demo", "-us", "region"),
        ];
        for (project, region, field) in cases {
            let body = json!({ "project": project, "region": region }).to_string();
            let res = call(&app, Method::POST, "/v1/environments", Some(&body)).await;
            expect_error(res, StatusCode::BAD_REQUEST, field).await;
        }
        let res = call(&app, Method::GET, "/v1/environments", None).await;
        assert!(read::<Vec<Environment>>(res).await.is_empty());
    }

    #[tokio::test]
    async fn create_rejects_malformed_bodies_with_json_errors() {
        let app = app();
        let res = call(&app, Method::POST, "/v1/environments", Some("{not json")).await;
        expect_error(res, StatusCode::BAD_REQUEST, "").await;

        let res = call(
            &app,
            Method::POST,
            "/v1/environments",
            Some(r#"{"project":"demo"}"#),
        )
        .await;
        expect_error(res, StatusCode::UNPROCESSABLE_ENTITY, "region").await;

        let res = call(
            &app,
            Method::POST,
            "/v1/environments",
            Some(r#"{"project":1,"region":"us-east"}"#),
        )
        .await;
        expect_error(res, StatusCode::UNPROCESSABLE_ENTITY, "project").await;

        let req = Request::post("/v1/environments")
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from(r#"{"project":"demo","region":"us-east"}"#))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        expect_error(res, StatusCode::UNSUPPORTED_MEDIA_TYPE, "").await;
    }

    #[tokio::test]
    async fn unknown_and_malformed_ids_are_rejected() {
        let app = app();
        let unknown = format!("/v1/environments/{}", Uuid::new_v4());
        for method in [Method::GET, Method::DELETE] {
            let res = call(&app, method.clone(), &unknown, None).await;
            expect_error(res, StatusCode::NOT_FOUND, "environment not found").await;

            let res = call(&app, method, "/v1/environments/not-a-uuid", None).await;
            expect_error(res, StatusCode::BAD_REQUEST, "UUID").await;
        }
    }

    #[tokio::test]
    async fn list_is_sorted_and_complete() {
        let app = app();
        let res = call(&app, Method::GET, "/v1/environments", None).await;
        assert!(read::<Vec<Environment>>(res).await.is_empty());

        let mut created = Vec::new();
        for project in ["a", "b", "c"] {
            created.push(create(&app, project, "us-east").await);
        }
        created.sort_by_key(|e| e.id);

        let res = call(&app, Method::GET, "/v1/environments", None).await;
        assert_eq!(read::<Vec<Environment>>(res).await, created);
    }

    #[tokio::test]
    async fn destroy_is_idempotent_and_visible() {
        let app = app();
        let env = create(&app, "demo", "us-east").await;
        let uri = format!("/v1/environments/{}", env.id);

        for _ in 0..2 {
            let res = call(&app, Method::DELETE, &uri, None).await;
            assert_eq!(res.status(), StatusCode::ACCEPTED);
            let destroyed: Environment = read(res).await;
            assert_eq!(destroyed.id, env.id);
            assert_eq!(destroyed.status, EnvironmentStatus::DestroyRequested);
        }

        let res = call(&app, Method::GET, &uri, None).await;
        assert_eq!(
            read::<Environment>(res).await.status,
            EnvironmentStatus::DestroyRequested
        );
    }

    #[tokio::test]
    async fn destroying_one_environment_leaves_others() {
        let app = app();
        let a = create(&app, "a", "us-east").await;
        let b = create(&app, "b", "us-east").await;
        call(
            &app,
            Method::DELETE,
            &format!("/v1/environments/{}", a.id),
            None,
        )
        .await;

        let res = call(
            &app,
            Method::GET,
            &format!("/v1/environments/{}", b.id),
            None,
        )
        .await;
        assert_eq!(
            read::<Environment>(res).await.status,
            EnvironmentStatus::Requested
        );
    }

    #[tokio::test]
    async fn unknown_routes_and_methods_are_rejected() {
        let app = app();
        let res = call(&app, Method::GET, "/v2/nope", None).await;
        expect_error(res, StatusCode::NOT_FOUND, "route not found").await;

        let res = call(&app, Method::PUT, "/v1/environments", Some("{}")).await;
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_creates_are_all_stored_with_unique_ids() {
        const N: usize = 64;
        let app = app();
        let tasks: Vec<_> = (0..N)
            .map(|i| {
                let app = app.clone();
                tokio::spawn(async move { create(&app, &format!("p{i}"), "us-east").await })
            })
            .collect();
        let mut ids = HashSet::new();
        for task in tasks {
            assert!(ids.insert(task.await.unwrap().id), "duplicate id");
        }

        let res = call(&app, Method::GET, "/v1/environments", None).await;
        let listed: HashSet<Uuid> = read::<Vec<Environment>>(res)
            .await
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(listed, ids);
    }

    #[test]
    fn bind_accepts_only_loopback() {
        assert!(parse_bind("127.0.0.1:8080").is_ok());
        assert!(parse_bind("127.0.0.1:0").is_ok());
        assert!(parse_bind("[::1]:8080").is_ok());
        assert!(parse_bind(DEFAULT_BIND).is_ok());

        for bad in [
            "0.0.0.0:8080",
            "[::]:8080",
            "192.168.1.10:8080",
            "localhost:8080",
            "8080",
            "",
        ] {
            assert!(parse_bind(bad).is_err(), "{bad:?} must be rejected");
        }
    }
}
