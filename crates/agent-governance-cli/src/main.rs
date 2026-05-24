use agent_governance_core::{
    create_context_envelope, default_council_pack, deliberate_with_pack, envelope_json,
    load_council_pack_from_path, persona_catalog_from_pack, planned_fanout,
    normalize_dev_mode, ContextEnvelopeRequest, CouncilPack, FanoutRequest, DEV_CONTEXT_SECRET,
};
use agent_governance_server::{serve, sqlite_database_url, AppConfig, DEFAULT_BIND_ADDR};
use anyhow::Context;
use clap::{Parser, Subcommand};
use std::{fs, net::SocketAddr, path::{Path, PathBuf}};

#[derive(Debug, Parser)]
#[command(name = "agent-governance")]
#[command(about = "Local CLI for agent-governance-rs")]
#[command(after_long_help = "Tip: run `agent-governance doctor` for example paths and copy-paste starters.\nHTTP examples: docs/api-cheatsheet.md (at the repository root).")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show version, example paths, and copy-paste starters
    Doctor,
    Personas {
        #[arg(long, env = "AGENT_GOV_COUNCIL_PACK")]
        council_pack: Option<PathBuf>,
    },
    Council {
        #[command(subcommand)]
        command: CouncilCommand,
    },
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    Fanout {
        #[command(subcommand)]
        command: FanoutCommand,
    },
    Server(ServerArgs),
}

#[derive(Debug, Subcommand)]
enum CouncilCommand {
    Run {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, env = "AGENT_GOV_COUNCIL_PACK")]
        council_pack: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    Sign {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, env = "AGENT_GOV_CONTEXT_SECRET")]
        secret: Option<String>,
        #[arg(long)]
        dev: bool,
    },
}

#[derive(Debug, Subcommand)]
enum FanoutCommand {
    Plan {
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Debug, Parser)]
struct ServerArgs {
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
    let cli = Cli::parse();
    match cli.command {
        Command::Doctor => run_doctor()?,
        Command::Personas { council_pack } => {
            print_json(&persona_catalog_from_pack(&load_pack(council_pack)?))?
        }
        Command::Council {
            command: CouncilCommand::Run { file, council_pack },
        } => {
            let req: agent_governance_core::CouncilRequest = read_json(&file)?;
            let pack = load_pack(council_pack)?;
            print_json(&deliberate_with_pack(req, &pack)?)?;
        }
        Command::Context {
            command: ContextCommand::Sign {
                file,
                secret,
                dev,
            },
        } => {
            let req: ContextEnvelopeRequest = read_json(&file)?;
            let dev_mode = normalize_dev_mode(dev);
            let secret = match (secret, dev_mode) {
                (Some(s), _) => s,
                (None, true) => DEV_CONTEXT_SECRET.to_string(),
                (None, false) => anyhow::bail!(
                    "pass --secret or enable dev placeholders (--dev / AGENT_GOV_DEV)",
                ),
            };
            let envelope = create_context_envelope(req, &secret)?;
            print_json(&envelope_json(&envelope))?;
        }
        Command::Fanout {
            command: FanoutCommand::Plan { file },
        } => {
            let req: FanoutRequest = read_json(&file)?;
            print_json(&planned_fanout(req))?;
        }
        Command::Server(args) => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .init();
            let database_url = args.db.map(sqlite_database_url);
            let dev_mode = normalize_dev_mode(args.dev);
            let context_secret = resolved_server_context_material(
                args.context_secret.clone(),
                args.token.clone(),
                dev_mode,
            )?;
            serve(
                AppConfig {
                    token: args.token,
                    dev_no_auth: args.dev_no_auth,
                    dev_mode,
                    context_secret,
                    database_url,
                    council_pack_path: args.council_pack,
                },
                args.bind,
            )
            .await?;
        }
    }
    Ok(())
}

fn run_doctor() -> anyhow::Result<()> {
    println!("agent-governance CLI {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Copy-paste (repository root, CLI only — no server):");
    println!("  cargo run -q -p agent-governance-cli --bin agent-governance -- personas");
    println!("  cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json");
    println!("  cargo run -q -p agent-governance-cli --bin agent-governance -- context sign --file examples/context-envelope/facts.json --dev");
    println!("  cargo run -q -p agent-governance-cli --bin agent-governance -- fanout plan --file examples/living-research/fanout.json");
    println!();
    println!("Dev server (defaults described in README and .env.example):");
    println!("  AGENT_GOV_DEV=1 AGENT_GOV_TOKEN=dev-token cargo run -p agent-governance-cli --bin agent-governance -- server --bind 127.0.0.1:9797 --dev --db ./agent-governance.sqlite");
    println!();
    println!("HTTP smoke check (no auth): curl -sS http://127.0.0.1:9797/health");
    println!("curl recipes: docs/api-cheatsheet.md");
    println!();
    println!("Environment variables: see .env.example");
    println!();
    let cwd = std::env::current_dir()?;
    print_repo_tree("examples", &cwd)?;
    print_repo_tree("packs", &cwd)?;
    Ok(())
}

fn print_repo_tree(subdir: &str, cwd: &Path) -> anyhow::Result<()> {
    let root = cwd.join(subdir);
    println!("{subdir}/");
    if !root.is_dir() {
        println!("  (missing — run `doctor` from the repository root to list bundled files)");
        println!();
        return Ok(());
    }
    let mut files = Vec::new();
    collect_files(&root, &mut files)?;
    files.sort();
    for path in files {
        if let Ok(rel) = path.strip_prefix(cwd) {
            println!("  {}", rel.display());
        }
    }
    println!();
    Ok(())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn resolved_server_context_material(
    context_secret_cli: Option<String>,
    token: Option<String>,
    dev_mode: bool,
) -> anyhow::Result<String> {
    Ok(match (context_secret_cli, token.clone()) {
        (Some(secret), _) => secret,
        (None, Some(t)) => t,
        (None, None) if dev_mode => DEV_CONTEXT_SECRET.to_string(),
        (None, None) => anyhow::bail!(
            "AGENT_GOV_CONTEXT_SECRET or AGENT_GOV_TOKEN required unless dev mode (--dev / AGENT_GOV_DEV)",
        ),
    })
}

fn load_pack(path: Option<PathBuf>) -> anyhow::Result<CouncilPack> {
    match path {
        Some(path) => Ok(load_council_pack_from_path(path)?),
        None => Ok(default_council_pack()),
    }
}

fn read_json<T: for<'de> serde::Deserialize<'de>>(path: &PathBuf) -> anyhow::Result<T> {
    let body = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&body).with_context(|| format!("parse {}", path.display()))
}

fn print_json<T: serde::Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
