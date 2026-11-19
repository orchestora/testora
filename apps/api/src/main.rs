//! Local-only prototype API. Do not expose publicly: no auth or tenant isolation.
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use std::{collections::HashMap, sync::Arc};
use testora_contracts::{
    ApiError, CreateEnvironmentRequest, Environment, EnvironmentStatus, HealthResponse,
};
use tokio::sync::RwLock;
use uuid::Uuid;

type Store = Arc<RwLock<HashMap<Uuid, Environment>>>;

fn app() -> Router {
    let store: Store = Arc::new(RwLock::new(HashMap::new()));
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
        .with_state(store)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        service: "testora-api".into(),
    })
}

async fn list_environments(State(store): State<Store>) -> Json<Vec<Environment>> {
    let mut envs: Vec<Environment> = store.read().await.values().cloned().collect();
    envs.sort_by_key(|env| env.id);
    Json(envs)
}

async fn create_environment(
    State(store): State<Store>,
    Json(req): Json<CreateEnvironmentRequest>,
) -> Result<(StatusCode, Json<Environment>), (StatusCode, Json<ApiError>)> {
    if req.project.trim().is_empty() || req.region.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "project and region must be nonempty".into(),
            }),
        ));
    }
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
    Path(id): Path<Uuid>,
) -> Result<Json<Environment>, (StatusCode, Json<ApiError>)> {
    store
        .read()
        .await
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(not_found)
}

async fn destroy_environment(
    State(store): State<Store>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<Environment>), (StatusCode, Json<ApiError>)> {
    let mut guard = store.write().await;
    let env = guard.get_mut(&id).ok_or_else(not_found)?;
    env.status = EnvironmentStatus::DestroyRequested;
    Ok((StatusCode::ACCEPTED, Json(env.clone())))
}

fn not_found() -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::NOT_FOUND,
        Json(ApiError {
            error: "environment not found".into(),
        }),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    println!("Testora local API listening at http://127.0.0.1:8080");
    axum::serve(listener, app()).await?;
    Ok(())
}
