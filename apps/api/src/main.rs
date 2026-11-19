//! Local-only demo API. Do not expose publicly: no auth or tenant isolation.

use anyhow::{Context, Result, anyhow};
use testora_api::{DEFAULT_BIND, app, parse_bind};

#[tokio::main]
async fn main() -> Result<()> {
    let bind = std::env::var("TESTORA_API_BIND").unwrap_or_else(|_| DEFAULT_BIND.into());
    let addr = parse_bind(&bind).map_err(|e| anyhow!("TESTORA_API_BIND: {e}"))?;
    let listener = tokio::net::TcpListener::bind(addr).await.with_context(|| {
        format!("could not listen on {addr}; set TESTORA_API_BIND to a free loopback port")
    })?;
    println!(
        "Testora local demo API listening at http://{}",
        listener.local_addr()?
    );
    axum::serve(listener, app())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
