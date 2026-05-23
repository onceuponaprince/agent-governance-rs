use agent_governance_core::{normalize_dev_mode, DEV_CONTEXT_SECRET};
use agent_governance_server::{serve, sqlite_database_url, AppConfig, DEFAULT_BIND_ADDR};
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Parser)]
#[command(name = "agent-governance-server")]
#[command(about = "HTTP server for agent-governance-rs")]
struct Args {
    #[arg(long, default_value = DEFAULT_BIND_ADDR)]
    bind: SocketAddr,
    #[arg(long, env = "AGENT_GOV_DB")]
    db: Option<PathBuf>,
    #[arg(long, env = "AGENT_GOV_TOKEN")]
    token: Option<String>,
    #[arg(long, env = "AGENT_GOV_CONTEXT_SECRET")]
    context_secret: Option<String>,
    #[arg(long, env = "AGENT_GOV_COUNCIL_PACK")]
    council_pack: Option<PathBuf>,
    #[arg(long)]
    dev_no_auth: bool,
    #[arg(long)]
    dev: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    let dev_mode = normalize_dev_mode(args.dev);

    serve(
        AppConfig {
            token: args.token.clone(),
            dev_no_auth: args.dev_no_auth,
            dev_mode,
            context_secret: resolve_context_material(
                args.context_secret.clone(),
                args.token.clone(),
                dev_mode,
            )?,
            database_url: args.db.map(sqlite_database_url),
            council_pack_path: args.council_pack,
        },
        args.bind,
    )
    .await
}

fn resolve_context_material(
    context_secret_cli: Option<String>,
    token: Option<String>,
    dev_mode: bool,
) -> anyhow::Result<String> {
    Ok(match (context_secret_cli, token.clone()) {
        (Some(secret), _) => secret,
        (None, Some(t)) => t,
        (None, None) if dev_mode => DEV_CONTEXT_SECRET.to_string(),
        (None, None) => anyhow::bail!(
            "AGENT_GOV_CONTEXT_SECRET or AGENT_GOV_TOKEN is required unless dev mode (--dev / AGENT_GOV_DEV)",
        ),
    })
}
