use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use reqwest::Client;
use testora_contracts::{CreateEnvironmentRequest, Environment, HealthResponse};
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "testora",
    version,
    about = "Testora local QA environment demo CLI"
)]
struct Cli {
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
    Create {
        #[arg(long)]
        project: String,
        #[arg(long, default_value = "us-east")]
        region: String,
    },
    List,
    Status {
        id: Uuid,
    },
    Destroy {
        id: Uuid,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let base = cli.api_url.trim_end_matches('/');
    // This HTTP default is deliberately local-only. Production must use HTTPS and auth.
    if !(base.starts_with("http://127.0.0.1:") || base.starts_with("http://localhost:")) {
        bail!("This demo CLI only supports localhost; production HTTPS/auth is not implemented");
    }
    let client = Client::new();
    match cli.command {
        Command::Ping => {
            let result: HealthResponse = client
                .get(format!("{base}/health"))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}: {}", result.service, result.status);
        }
        Command::Env { action } => match action {
            EnvCommand::Create { project, region } => {
                let result: Environment = client
                    .post(format!("{base}/v1/environments"))
                    .json(&CreateEnvironmentRequest { project, region })
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                println!(
                    "{} {:?} (local demo: no cloud resources created)",
                    result.id, result.status
                );
            }
            EnvCommand::List => {
                let result: Vec<Environment> = client
                    .get(format!("{base}/v1/environments"))
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                for e in result {
                    println!("{}  {}  {}  {:?}", e.id, e.project, e.region, e.status);
                }
            }
            EnvCommand::Status { id } => {
                let e: Environment = client
                    .get(format!("{base}/v1/environments/{id}"))
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await
                    .context("Could not fetch environment")?;
                println!("{} {} {} {:?}", e.id, e.project, e.region, e.status);
            }
            EnvCommand::Destroy { id } => {
                let e: Environment = client
                    .delete(format!("{base}/v1/environments/{id}"))
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                println!(
                    "{} {:?} (local demo: no cloud resources existed)",
                    e.id, e.status
                );
            }
        },
    }
    Ok(())
}
