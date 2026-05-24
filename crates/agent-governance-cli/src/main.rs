use agent_governance_core::{
    create_context_envelope, default_council_pack, deliberate_with_pack, envelope_json,
    load_council_pack_from_path, persona_catalog_from_pack, planned_fanout, ContextEnvelopeRequest,
    CouncilPack, DeterministicEmbeddingProvider, EmbeddingProvider, FanoutRequest,
    HttpEmbeddingProvider, JsonlStore, ReasoningTrace,
};
use agent_governance_server::{serve, sqlite_database_url_from_path, AppConfig, DEFAULT_BIND_ADDR};
use anyhow::Context;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{
    generate,
    shells::{Bash, Elvish, Fish, PowerShell, Zsh},
};
use serde::Serialize;
use serde_json::json;
use std::{
    env, fs, io, net::SocketAddr, path::Path, path::PathBuf, process::Command as ProcessCommand,
    str::FromStr,
};

#[derive(Debug, Parser)]
#[command(name = "agent-governance")]
#[command(about = "Local CLI for agent-governance-rs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the persona catalog from the configured council pack.
    ///
    /// Example: cargo run -q -p agent-governance-cli --bin agent-governance -- personas --council-pack ./packs/default/council.json
    Personas {
        #[arg(long, env = "AGENT_GOV_COUNCIL_PACK")]
        council_pack: Option<PathBuf>,
    },

    /// Run council deliberation from a request file and emit prompts for positions.
    ///
    /// Example: cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json
    Council {
        #[command(subcommand)]
        command: CouncilCommand,
    },

    /// Build or validate context envelopes.
    ///
    /// Example: cargo run -q -p agent-governance-cli --bin agent-governance -- context sign --file examples/context-envelope/facts.json
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },

    /// Plan fanout across adapter lanes (no external calls are executed here).
    ///
    /// Example: cargo run -q -p agent-governance-cli --bin agent-governance -- fanout plan --file examples/living-research/fanout.json
    Fanout {
        #[command(subcommand)]
        command: FanoutCommand,
    },

    /// Print and describe the first-run operating flow.
    Quickstart,

    /// Validate local environment and command prerequisites.
    ///
    /// Example: cargo run -q -p agent-governance-cli --bin agent-governance -- doctor
    Doctor {
        /// Emit a JSON health summary instead of human-friendly text.
        #[arg(long)]
        json: bool,
    },

    /// Generate shell completion.
    ///
    /// Example: cargo run -q -p agent-governance-cli --bin agent-governance -- completion bash > /tmp/agent-governance.bash
    Completion {
        #[arg(value_enum)]
        shell: CompletionShell,
        /// Binary name used inside generated completion scripts.
        #[arg(long, default_value_t = String::from("agent-governance"))]
        bin_name: String,
    },

    /// Start the local HTTP server.
    ///
    /// Example: AGENT_GOV_TOKEN=dev-token cargo run -q -p agent-governance-cli --bin agent-governance -- server --bind 127.0.0.1:9797 --db ./agent-governance.sqlite
    Server(ServerArgs),

    /// Work with local reasoning traces.
    ///
    /// Example: cargo run -q -p agent-governance-cli --bin agent-governance -- reasoning list --store traces.jsonl
    Reasoning {
        #[command(subcommand)]
        command: ReasoningCommand,
    },

    /// Generate JSON schemas for request bodies and adapters.
    ///
    /// Example: cargo run -q -p agent-governance-cli --bin agent-governance -- schema generate --out-dir schemas
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
    Generate {
        #[arg(long, default_value = "schemas")]
        out_dir: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ReasoningCommand {
    Ingest {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value = "traces.jsonl")]
        store: PathBuf,
    },
    List {
        #[arg(long, default_value = "traces.jsonl")]
        store: PathBuf,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Show {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "traces.jsonl")]
        store: PathBuf,
    },
    SqliteIngest {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value = "sqlite:./traces.sqlite")]
        db: String,
    },
    SqliteList {
        #[arg(long, default_value = "sqlite:./traces.sqlite")]
        db: String,
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },
    SqliteQueryTag {
        #[arg(long)]
        tag: String,
        #[arg(long, default_value = "sqlite:./traces.sqlite")]
        db: String,
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },
    SqliteSearchVec {
        #[arg(long)]
        embedding_file: PathBuf,
        #[arg(long, default_value = "sqlite:./traces.sqlite")]
        db: String,
        #[arg(long, default_value_t = 5)]
        top: usize,
    },
}
#[derive(Debug, Subcommand)]
enum CouncilCommand {
    Run {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, env = "AGENT_GOV_COUNCIL_PACK")]
        council_pack: Option<PathBuf>,
    },
    SubmitPositions {
        /// Path to an existing council run JSON file (or use --db/--id to fetch from DB)
        #[arg(long)]
        file: Option<PathBuf>,
        /// Positions JSON file (array of CouncilMemberPosition)
        #[arg(long)]
        positions_file: PathBuf,
        /// Optional sqlite DB url to read/write deliberations: e.g. sqlite:./agent-governance.sqlite
        #[arg(long)]
        db: Option<String>,
        /// When using --db and --id, specify the deliberation id to update
        #[arg(long)]
        id: Option<String>,
        /// Output path for updated run JSON (default stdout)
        #[arg(long)]
        out: Option<PathBuf>,
        /// If set and --db is provided, commit updated run back into DB
        #[arg(long)]
        commit: bool,
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

#[derive(Debug, Clone, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Personas { council_pack } => {
            print_json(&persona_catalog_from_pack(&load_pack(council_pack)?))?
        }
        Command::Council { command } => match command {
            CouncilCommand::Run { file, council_pack } => {
                let req: agent_governance_core::CouncilRequest = read_json(&file)?;
                let pack = load_pack(council_pack)?;
                print_json(&deliberate_with_pack(req, &pack)?)?;
            }
            CouncilCommand::SubmitPositions {
                file,
                positions_file,
                db,
                id,
                out,
                commit,
                council_pack,
            } => {
                let positions: Vec<agent_governance_core::CouncilMemberPosition> =
                    read_json(&positions_file)?;
                let pack = load_pack(council_pack)?;
                if let Some(db_url) = db {
                    if id.is_none() {
                        anyhow::bail!("--id is required when --db is provided");
                    }
                    let url = if db_url.starts_with("sqlite:") {
                        db_url
                    } else {
                        format!("sqlite:{}", db_url)
                    };
                    let options = sqlx::sqlite::SqliteConnectOptions::from_str(&url)?;
                    let pool = sqlx::sqlite::SqlitePoolOptions::new()
                        .connect_with(options)
                        .await?;
                    let json_text: Option<String> = sqlx::query_scalar::<_, String>(
                        "select json from deliberations where id = ?1 order by rowid desc limit 1",
                    )
                    .bind(id.clone().unwrap())
                    .fetch_optional(&pool)
                    .await?;
                    if let Some(text) = json_text {
                        let run: agent_governance_core::CouncilRun = serde_json::from_str(&text)?;
                        let request = agent_governance_core::CouncilRequest {
                            title: run.title.clone(),
                            problem: run.problem.clone(),
                            mode: run.mode.clone(),
                            domain: Some(run.domain.clone()),
                            members: None,
                            llms: vec![],
                            default_llm: None,
                            targets: vec![],
                            consensus_threshold: run.consensus.threshold,
                            positions,
                        };
                        let new_run = deliberate_with_pack(request, &pack)?;
                        if commit {
                            sqlx::query(
                                    "insert into deliberations (id, json, report_md, created_at) values (?1, ?2, ?3, ?4)",
                                )
                                .bind(new_run.council_id.to_string())
                                .bind(serde_json::to_string(&new_run)?)
                                .bind(agent_governance_core::render_council_report(&new_run))
                                .bind(new_run.created_at.to_rfc3339())
                                .execute(&pool)
                                .await?;
                        }
                        if let Some(path) = out {
                            std::fs::write(path, serde_json::to_string_pretty(&new_run)?)?;
                        } else {
                            print_json(&new_run)?;
                        }
                    } else {
                        anyhow::bail!("deliberation not found: {}", id.unwrap());
                    }
                } else if let Some(file) = file {
                    let run: agent_governance_core::CouncilRun = read_json(&file)?;
                    let request = agent_governance_core::CouncilRequest {
                        title: run.title.clone(),
                        problem: run.problem.clone(),
                        mode: run.mode.clone(),
                        domain: Some(run.domain.clone()),
                        members: None,
                        llms: vec![],
                        default_llm: None,
                        targets: vec![],
                        consensus_threshold: run.consensus.threshold,
                        positions,
                    };
                    let new_run = deliberate_with_pack(request, &pack)?;
                    if let Some(path) = out {
                        std::fs::write(path, serde_json::to_string_pretty(&new_run)?)?;
                    } else {
                        print_json(&new_run)?;
                    }
                } else {
                    anyhow::bail!("either --file or --db/--id must be provided");
                }
            }
        },
        Command::Context {
            command: ContextCommand::Sign { file, secret },
        } => {
            let req: ContextEnvelopeRequest = read_json(&file)?;
            let secret = secret.unwrap_or_else(|| "dev-context-secret".to_string());
            let envelope = create_context_envelope(req, &secret)?;
            print_json(&envelope_json(&envelope))?;
        }
        Command::Fanout {
            command: FanoutCommand::Plan { file },
        } => {
            let req: FanoutRequest = read_json(&file)?;
            print_json(&planned_fanout(req))?;
        }
        Command::Quickstart => {
            print_quickstart();
        }
        Command::Doctor { json } => {
            run_doctor_checks(json)?;
        }
        Command::Completion { shell, bin_name } => {
            let mut command = Cli::command();
            let mut stdout = io::stdout();
            match shell {
                CompletionShell::Bash => generate(Bash, &mut command, &bin_name, &mut stdout),
                CompletionShell::Zsh => generate(Zsh, &mut command, &bin_name, &mut stdout),
                CompletionShell::Fish => generate(Fish, &mut command, &bin_name, &mut stdout),
                CompletionShell::PowerShell => {
                    generate(PowerShell, &mut command, &bin_name, &mut stdout)
                }
                CompletionShell::Elvish => generate(Elvish, &mut command, &bin_name, &mut stdout),
            };
        }
        Command::Reasoning { command } => match command {
            ReasoningCommand::Ingest { file, store } => {
                let mut trace: ReasoningTrace = read_json(&file)?;
                // compute embedding at ingest when missing
                if trace.embedding.is_none() {
                    let provider = embedding_provider_from_env(128);
                    trace.embedding = Some(provider.embed(&trace.prompt).await?);
                }
                let s = JsonlStore::new(store);
                s.append(&trace)?;
                println!("ingested: {}", trace.id);
            }
            ReasoningCommand::List { store, limit } => {
                let s = JsonlStore::new(store);
                let mut traces = s.read_all()?;
                traces.truncate(limit);
                print_json(&traces)?;
            }
            ReasoningCommand::Show { id, store } => {
                let s = JsonlStore::new(store);
                let traces = s.read_all()?;
                let found = traces.into_iter().find(|t| t.id.to_string() == id);
                match found {
                    Some(t) => print_json(&t)?,
                    None => println!("not found: {}", id),
                }
            }
            ReasoningCommand::SqliteIngest { file, db } => {
                let mut trace: ReasoningTrace = read_json(&file)?;
                if trace.embedding.is_none() {
                    let provider = embedding_provider_from_env(128);
                    trace.embedding = Some(provider.embed(&trace.prompt).await?);
                }
                let url = if db.starts_with("sqlite:") {
                    db
                } else {
                    format!("sqlite:{}", db)
                };
                let store = agent_governance_core::SqliteReasoningStore::new(&url).await?;
                store.insert(&trace).await?;
                println!("ingested: {}", trace.id);
            }
            ReasoningCommand::SqliteList { db, limit } => {
                let url = if db.starts_with("sqlite:") {
                    db
                } else {
                    format!("sqlite:{}", db)
                };
                let store = agent_governance_core::SqliteReasoningStore::new(&url).await?;
                let traces = store.list(limit).await?;
                print_json(&traces)?;
            }
            ReasoningCommand::SqliteQueryTag { tag, db, limit } => {
                let url = if db.starts_with("sqlite:") {
                    db
                } else {
                    format!("sqlite:{}", db)
                };
                let store = agent_governance_core::SqliteReasoningStore::new(&url).await?;
                let traces = store.query_by_tag(&tag, limit).await?;
                print_json(&traces)?;
            }
            ReasoningCommand::SqliteSearchVec {
                embedding_file,
                db,
                top,
            } => {
                let raw = fs::read_to_string(&embedding_file)?;
                let query: Vec<f32> = serde_json::from_str(&raw)
                    .with_context(|| format!("read embedding {}", embedding_file.display()))?;
                let url = if db.starts_with("sqlite:") {
                    db
                } else {
                    format!("sqlite:{}", db)
                };
                let store = agent_governance_core::SqliteReasoningStore::new(&url).await?;
                let found = store.search_by_embedding(&query, top).await?;
                // print as array of {trace, score}
                let out: Vec<_> = found
                    .into_iter()
                    .map(|(t, s)| json!({"score": s, "trace": t}))
                    .collect();
                print_json(&out)?;
            }
        },
        Command::Schema { command } => match command {
            SchemaCommand::Generate { out_dir } => {
                fs::create_dir_all(&out_dir)?;
                write_schema_file(
                    &out_dir.join("council-request.schema.json"),
                    json!({
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "title": "CouncilRequest",
                        "type": "object",
                        "required": ["problem"],
                        "properties": {
                            "title": {"type": ["string", "null"]},
                            "problem": {"type": "string"},
                            "mode": {"type": "string", "enum": ["quick", "duo", "full"]},
                            "domain": {"type": ["string", "null"]},
                            "consensus_threshold": {"type": "number"},
                            "positions": {"type": "array"}
                        }
                    }),
                )?;
                write_schema_file(
                    &out_dir.join("fanout-request.schema.json"),
                    json!({
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "title": "FanoutRequest",
                        "type": "object",
                        "required": ["goal"],
                        "properties": {
                            "goal": {"type": "string"},
                            "sources": {"type": "array"}
                        }
                    }),
                )?;
                write_schema_file(
                    &out_dir.join("memory-fact-request.schema.json"),
                    json!({
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "title": "MemoryFactRequest",
                        "type": "object",
                        "required": ["fact", "source"],
                        "properties": {
                            "fact": {"type": "string"},
                            "source": {"type": "string"},
                            "ttl_seconds": {"type": "integer"},
                            "relevance": {"type": "number"}
                        }
                    }),
                )?;
                write_schema_file(
                    &out_dir.join("tool-result-request.schema.json"),
                    json!({
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "title": "ToolResultRequest",
                        "type": "object",
                        "required": ["name", "ok"],
                        "properties": {
                            "name": {"type": "string"},
                            "ok": {"type": "boolean"},
                            "latency_ms": {"type": "integer"},
                            "cost": {"type": "number"},
                            "risk": {"type": "number"}
                        }
                    }),
                )?;
                println!("generated schemas in {}", out_dir.display());
            }
        },
        Command::Server(args) => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .init();
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
            .await?;
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    check: String,
    status: String,
    details: String,
    hint: Option<String>,
}

fn doctor_check_binary(name: &str, version_flag: &[&str], required: bool) -> DoctorCheck {
    let command = ProcessCommand::new(name).args(version_flag).output();
    let (status, output) = match command {
        Ok(output) if output.status.success() => (
            "pass",
            Some(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
            ),
        ),
        Ok(_) => (
            "warn",
            Some(format!("command exists but could not run {name}")),
        ),
        Err(_) => (
            if required { "fail" } else { "warn" },
            Some(format!("command {name} not found in PATH")),
        ),
    };
    let output_ref = output.as_deref().unwrap_or(status).to_string();
    DoctorCheck {
        check: format!("{name} command"),
        status: status.to_string(),
        details: output_ref,
        hint: output,
    }
}

fn doctor_check_file(path: &str, description: &str) -> DoctorCheck {
    let status = if Path::new(path).exists() {
        "pass"
    } else {
        "fail"
    };
    DoctorCheck {
        check: description.to_string(),
        status: status.to_string(),
        details: path.to_string(),
        hint: if status == "pass" {
            None
        } else {
            Some(format!("required file missing: {path}"))
        },
    }
}

fn doctor_check_workspace() -> DoctorCheck {
    let status = if Path::new("target").exists() {
        "warn"
    } else {
        "pass"
    };
    let hint = if status == "warn" {
        Some(String::from("remove target/ before release-qa snapshots"))
    } else {
        None
    };
    DoctorCheck {
        check: String::from("target directory"),
        status: status.to_string(),
        details: String::from("target/ must be absent for release-style checks"),
        hint,
    }
}

fn run_doctor_checks(json: bool) -> anyhow::Result<()> {
    let mut checks = vec![
        doctor_check_binary("cargo", &["--version"], true),
        doctor_check_binary("rustc", &["--version"], true),
        doctor_check_binary("curl", &["--version"], true),
        doctor_check_binary("gh", &["--version"], false),
        doctor_check_binary("sqlite3", &["--version"], false),
    ];
    checks.push(doctor_check_workspace());
    checks.extend([
        doctor_check_file("README.md", "README present"),
        doctor_check_file("docs/qa-guide.md", "QA guide present"),
        doctor_check_file("docs/usability-qa-guide.md", "Usability QA guide present"),
        doctor_check_file("scripts/qa-all.sh", "qa-all script present"),
    ]);

    if let Ok(db) = env::var("AGENT_GOV_DB") {
        let db_path = Path::new(&db);
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                checks.push(DoctorCheck {
                    check: String::from("database parent directory"),
                    status: String::from("warn"),
                    details: db,
                    hint: Some(String::from(
                        "create parent directory or use --db inside an existing folder",
                    )),
                });
            } else {
                checks.push(DoctorCheck {
                    check: String::from("database path writable"),
                    status: String::from("pass"),
                    details: db,
                    hint: None,
                });
            }
        }
    }

    let passed = checks.iter().filter(|c| c.status == "pass").count();
    let failed = checks.iter().filter(|c| c.status == "fail").count();
    let warned = checks.iter().filter(|c| c.status == "warn").count();

    if json {
        print_json(&json!({
            "checks": checks,
            "summary": json!({
                "passed": passed,
                "failed": failed,
                "warned": warned,
            })
        }))?;
        return if failed == 0 {
            Ok(())
        } else {
            anyhow::bail!("doctor reported {failed} failures")
        };
    }

    for check in &checks {
        let status = check.status.to_uppercase();
        println!("[{status}] {} => {}", check.check, check.details);
        if let Some(hint) = &check.hint {
            println!("    hint: {hint}");
        }
    }
    println!("doctor summary: passed={passed}, warned={warned}, failed={failed}");

    if failed > 0 {
        anyhow::bail!("doctor reported {failed} failures")
    }

    Ok(())
}

fn print_quickstart() {
    let evidence_dir = env::var("QA_EVIDENCE")
        .unwrap_or_else(|_| "/tmp/agent-governance-rs-v0.1.0-qa".to_string());
    println!("Quickstart flow:");
    println!("  1) mkdir -p {evidence_dir}");
    println!("  2) cargo run -q -p agent-governance-cli --bin agent-governance -- personas");
    println!("  3) cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json");
    println!("  4) cargo run -q -p agent-governance-cli --bin agent-governance -- context sign --file examples/context-envelope/facts.json");
    println!("  5) cargo run -q -p agent-governance-cli --bin agent-governance -- fanout plan --file examples/living-research/fanout.json");
    println!("  6) AGENT_GOV_TOKEN=dev-token cargo run -q -p agent-governance-cli --bin agent-governance -- server --bind 127.0.0.1:9797 --db {}/agent-governance.sqlite", evidence_dir);
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

fn embedding_provider_from_env(dim: usize) -> Box<dyn EmbeddingProvider> {
    let endpoint = std::env::var("AGENT_GOV_EMBED_ENDPOINT").ok();
    let api_key = std::env::var("AGENT_GOV_EMBED_API_KEY").ok();
    if let Some(endpoint) = endpoint {
        Box::new(HttpEmbeddingProvider::new(endpoint, api_key, dim))
    } else {
        Box::new(DeterministicEmbeddingProvider::new(dim))
    }
}

fn write_schema_file(path: &PathBuf, schema: serde_json::Value) -> anyhow::Result<()> {
    fs::write(path, serde_json::to_string_pretty(&schema)?)?;
    Ok(())
}
