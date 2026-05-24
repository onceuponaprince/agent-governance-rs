use agent_governance_server::{serve, sqlite_database_url_from_path, AppConfig, DEFAULT_BIND_ADDR};
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
    #[arg(long, env = "AGENT_GOV_HYDRATE_STATE", default_value_t = false)]
    hydrate_state_from_db: bool,
    #[arg(long, env = "AGENT_GOV_EMBED_ENDPOINT")]
    embedding_endpoint: Option<String>,
    #[arg(long, env = "AGENT_GOV_EMBED_API_KEY")]
    embedding_api_key: Option<String>,
    #[arg(long, env = "AGENT_GOV_EMBED_DIM", default_value_t = 128)]
    embedding_dim: usize,
    #[arg(long)]
    dev_no_auth: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    let database_url = args.db.map(sqlite_database_url_from_path).transpose()?;
    let context_secret = args
        .context_secret
        .or_else(|| args.token.clone())
        .unwrap_or_else(|| "dev-context-secret".to_string());
    serve(
        AppConfig {
            token: args.token,
            dev_no_auth: args.dev_no_auth,
            context_secret,
            database_url,
            council_pack_path: args.council_pack,
            hydrate_state_from_db: args.hydrate_state_from_db,
            embedding_endpoint: args.embedding_endpoint,
            embedding_api_key: args.embedding_api_key,
            embedding_dim: args.embedding_dim,
        },
        args.bind,
    )
    .await
}
