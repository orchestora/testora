use std::{net::IpAddr, time::Duration};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use reqwest::{Client, RequestBuilder, Url, header::CONTENT_TYPE};
use serde::de::DeserializeOwned;
use testora_contracts::{
    ApiError, CreateEnvironmentRequest, Environment, HealthResponse, validate_project,
    validate_region,
};
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "testora",
    version,
    about = "Testora local QA environment demo CLI"
)]
struct Cli {
    /// Demo API base URL. Plain HTTP on a loopback host only.
    #[arg(
        long,
        env = "TESTORA_API_URL",
        default_value = "http://127.0.0.1:8080",
        global = true
    )]
    api_url: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check whether the API is reachable.
    Ping,
    /// Manage local demo environment records.
    Env {
        #[command(subcommand)]
        action: EnvCommand,
    },
}

#[derive(Subcommand)]
enum EnvCommand {
    /// Request a new environment.
    Create {
        #[arg(long, value_parser = parse_project)]
        project: String,
        #[arg(long, default_value = "us-east", value_parser = parse_region)]
        region: String,
    },
    /// List environments.
    List,
    /// Show one environment.
    Status { id: Uuid },
    /// Request destruction of an environment. Safe to repeat.
    Destroy { id: Uuid },
}

fn parse_project(value: &str) -> Result<String, String> {
    validate_project(value).map_err(|e| e.message)?;
    Ok(value.to_owned())
}

fn parse_region(value: &str) -> Result<String, String> {
    validate_region(value).map_err(|e| e.message)?;
    Ok(value.to_owned())
}

/// Validate the API URL. The demo has no auth or TLS, so only plain HTTP to
/// a loopback host is allowed. Parsing (instead of prefix matching) rejects
/// tricks like `http://127.0.0.1:8080@evil.example`, where the real host is
/// `evil.example`.
fn api_base(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).with_context(|| format!("invalid API URL {raw:?}"))?;
    if url.scheme() != "http" {
        bail!("API URL must use http:// (the demo API has no TLS); got {raw:?}");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("API URL must not contain credentials; got {raw:?}");
    }
    let loopback = match url.host() {
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
        Some(url::Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
        None => false,
    };
    if !loopback {
        bail!(
            "API URL must point to a local API (localhost, 127.0.0.0/8 or ::1); this demo CLI never talks to remote hosts; got {raw:?}"
        );
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        bail!("API URL must be a bare base URL like http://127.0.0.1:8080; got {raw:?}");
    }
    Ok(url)
}

/// Send a request and decode the JSON body, turning API errors into
/// readable messages.
async fn send<T: DeserializeOwned>(request: RequestBuilder) -> Result<T> {
    let response = request.send().await.map_err(|e| {
        if e.is_connect() {
            anyhow::anyhow!("could not reach the Testora API ({e}); is `testora-api` running?")
        } else if e.is_timeout() {
            anyhow::anyhow!("the Testora API did not respond in time")
        } else {
            anyhow::Error::new(e).context("request to the Testora API failed")
        }
    })?;
    let status = response.status();
    if status.is_success() {
        return response
            .json()
            .await
            .context("the API returned an unexpected response body");
    }
    let is_json = response
        .headers()
        .get(CONTENT_TYPE)
        .is_some_and(|v| v.as_bytes().starts_with(b"application/json"));
    let detail = if is_json {
        response.json::<ApiError>().await.ok().map(|e| e.error)
    } else {
        None
    };
    match detail {
        Some(message) => bail!("API error ({status}): {message}"),
        None => bail!("API error ({status})"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let base = api_base(&cli.api_url)?;
    let base = base.as_str().trim_end_matches('/');
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .build()?;

    match cli.command {
        Command::Ping => {
            let health: HealthResponse = send(client.get(format!("{base}/health"))).await?;
            println!("{}: {}", health.service, health.status);
        }
        Command::Env { action } => match action {
            EnvCommand::Create { project, region } => {
                let request = CreateEnvironmentRequest { project, region };
                let env: Environment = send(
                    client
                        .post(format!("{base}/v1/environments"))
                        .json(&request),
                )
                .await?;
                println!(
                    "{} {:?} (local demo: no cloud resources created)",
                    env.id, env.status
                );
            }
            EnvCommand::List => {
                let envs: Vec<Environment> =
                    send(client.get(format!("{base}/v1/environments"))).await?;
                for e in envs {
                    println!("{}  {}  {}  {:?}", e.id, e.project, e.region, e.status);
                }
            }
            EnvCommand::Status { id } => {
                let e: Environment =
                    send(client.get(format!("{base}/v1/environments/{id}"))).await?;
                println!("{} {} {} {:?}", e.id, e.project, e.region, e.status);
            }
            EnvCommand::Destroy { id } => {
                let e: Environment =
                    send(client.delete(format!("{base}/v1/environments/{id}"))).await?;
                println!(
                    "{} {:?} (local demo: no cloud resources existed)",
                    e.id, e.status
                );
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn api_base_accepts_loopback_http() {
        for ok in [
            "http://127.0.0.1:8080",
            "http://127.0.0.1:8080/",
            "http://localhost:9000",
            "http://LOCALHOST:9000",
            "http://127.0.0.2:8080",
            "http://[::1]:8080",
        ] {
            assert!(api_base(ok).is_ok(), "{ok:?} should be accepted");
        }
    }

    #[test]
    fn api_base_rejects_remote_or_disguised_hosts() {
        for bad in [
            "http://127.0.0.1:8080@evil.example",
            "http://localhost:8080@evil.example/",
            "http://user:pass@127.0.0.1:8080",
            "http://127.0.0.1.evil.example:8080",
            "http://localhost.evil.example:8080",
            "http://10.0.0.1:8080",
            "http://0.0.0.0:8080",
            "http://api.example.com",
            "https://127.0.0.1:8080",
            "ftp://127.0.0.1:8080",
            "http://127.0.0.1:8080/v1",
            "http://127.0.0.1:8080/?x=1",
            "127.0.0.1:8080",
            "not a url",
            "",
        ] {
            assert!(api_base(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn create_arguments_are_validated_before_any_request() {
        let parse = |args: &[&str]| Cli::try_parse_from(args).map(|_| ());
        assert!(parse(&["testora", "env", "create", "--project", "demo"]).is_ok());
        assert!(parse(&["testora", "env", "create", "--project", "demo app"]).is_err());
        assert!(parse(&["testora", "env", "create", "--project", ""]).is_err());
        assert!(
            parse(&[
                "testora",
                "env",
                "create",
                "--project",
                "demo",
                "--region",
                "US_EAST"
            ])
            .is_err()
        );
        assert!(parse(&["testora", "env", "status", "not-a-uuid"]).is_err());
    }
}
