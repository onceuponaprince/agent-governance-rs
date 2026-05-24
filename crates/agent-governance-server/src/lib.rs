use agent_governance_core::{
    active_memory_facts, add_memory_fact, create_context_envelope, create_repair_plan,
    default_council_pack, deliberate_with_pack, invalidate_source, load_council_pack_from_path,
    persona_catalog_from_pack, planned_fanout, rank_tools, record_tool_result,
    render_council_report, render_fanout_report, stale_memory_facts, tool_state, ContextEnvelope,
    ContextEnvelopeRequest, ContextVerifyResponse, CouncilPack, DeterministicEmbeddingProvider,
    EmbeddingProvider, FanoutRun, HttpEmbeddingProvider, MemoryFactRecord, MemoryStore, RepairPlan,
    SourceInvalidationRecord, ToolResultRequest, ToolStore,
};
use anyhow::Context;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    str::FromStr,
    sync::Arc,
};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:9797";

pub fn sqlite_database_url(path: impl AsRef<FsPath>) -> String {
    format!("sqlite://{}", path.as_ref().display())
}

pub fn sqlite_database_url_from_path(path: impl AsRef<FsPath>) -> anyhow::Result<String> {
    let path = normalize_sqlite_path(path.as_ref())?;
    Ok(sqlite_database_url(&path))
}

fn normalize_sqlite_path(path: &FsPath) -> anyhow::Result<PathBuf> {
    let normalized = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };

    if normalized.as_os_str().is_empty() {
        anyhow::bail!("database path must not be empty");
    }

    if normalized.exists() && normalized.is_dir() {
        anyhow::bail!(
            "database path must be a file, not a directory: {}",
            normalized.display()
        );
    }

    let parent = normalized
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| FsPath::new("."));

    if !parent.exists() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create database parent directory {}", parent.display()))?;
    }

    if !parent.is_dir() {
        anyhow::bail!("database parent is not a directory: {}", parent.display());
    }

    let probe = parent.join(format!(
        ".agent-governance-rs-db-write-probe-{}",
        std::process::id()
    ));
    let mut probe_file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
        .with_context(|| {
            format!(
                "database parent {} is not writable; create or fix directory permissions, or use another parent directory",
                parent.display()
            )
        })?;
    probe_file.write_all(b"ok")?;
    probe_file.flush()?;
    drop(probe_file);
    fs::remove_file(&probe)
        .with_context(|| format!("remove write probe file {}", probe.display()))?;

    Ok(normalized)
}

#[derive(Clone)]
pub struct AppConfig {
    pub token: Option<String>,
    pub dev_no_auth: bool,
    pub context_secret: String,
    pub database_url: Option<String>,
    pub council_pack_path: Option<PathBuf>,
    pub hydrate_state_from_db: bool,
    pub embedding_endpoint: Option<String>,
    pub embedding_api_key: Option<String>,
    pub embedding_dim: usize,
}

#[derive(Clone)]
pub struct AppState {
    config: AppConfig,
    db: Option<SqlitePool>,
    council_pack: CouncilPack,
    memory: Arc<Mutex<MemoryStore>>,
    tools: Arc<Mutex<ToolStore>>,
    councils: Arc<Mutex<BTreeMap<Uuid, agent_governance_core::CouncilRun>>>,
    fanouts: Arc<Mutex<BTreeMap<String, FanoutRun>>>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
}

pub async fn app(config: AppConfig) -> anyhow::Result<Router> {
    let council_pack = if let Some(path) = &config.council_pack_path {
        load_council_pack_from_path(path)?
    } else {
        default_council_pack()
    };
    let db = if let Some(url) = &config.database_url {
        let options = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        init_db(&pool).await?;
        Some(pool)
    } else {
        None
    };
    let embedding_provider: Arc<dyn EmbeddingProvider> =
        if let Some(endpoint) = &config.embedding_endpoint {
            Arc::new(HttpEmbeddingProvider::new(
                endpoint.clone(),
                config.embedding_api_key.clone(),
                config.embedding_dim,
            ))
        } else {
            Arc::new(DeterministicEmbeddingProvider::new(config.embedding_dim))
        };

    let state = AppState {
        config,
        db,
        council_pack,
        memory: Arc::new(Mutex::new(MemoryStore::default())),
        tools: Arc::new(Mutex::new(ToolStore::default())),
        councils: Arc::new(Mutex::new(BTreeMap::new())),
        fanouts: Arc::new(Mutex::new(BTreeMap::new())),
        embedding_provider,
    };

    if state.config.hydrate_state_from_db {
        hydrate_operational_state_from_db(&state).await?;
    }

    Ok(Router::new()
        .route("/health", get(health))
        .route("/v1/council/personas", get(council_personas))
        .route(
            "/v1/council/deliberations",
            post(create_deliberation).get(list_recent_deliberations),
        )
        .route("/v1/council/deliberations/{id}", get(get_deliberation))
        .route(
            "/v1/council/deliberations/{id}/report.md",
            get(get_deliberation_report),
        )
        .route("/v1/context/envelopes", post(create_envelope))
        .route("/v1/context/envelopes/verify", post(verify_envelope))
        .route(
            "/v1/memory/facts",
            post(create_memory_fact).get(list_memory_facts),
        )
        .route("/v1/memory/invalidations", post(create_memory_invalidation))
        .route("/v1/tools/results", post(create_tool_result))
        .route("/v1/tools/rank", get(get_tool_rank))
        .route("/v1/tools/{name}", get(get_tool_state))
        .route("/v1/repair/plans", post(create_repair_plan_route))
        .route(
            "/v1/fanout/plans",
            post(create_fanout_plan).get(list_recent_fanout_plans),
        )
        .route("/v1/fanout/plans/{id}", get(get_fanout_plan))
        .route("/v1/fanout/plans/{id}/report.md", get(get_fanout_report))
        .route("/v1/reasoning/traces", post(create_reasoning_trace))
        .route("/v1/reasoning/search", post(search_reasoning_traces))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http()))
}

fn parse_request_json<T: for<'de> Deserialize<'de>>(
    body: &str,
    context: &'static str,
) -> Result<T, ApiError> {
    serde_json::from_str::<T>(body).map_err(|err| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            format!("{}: {err}", context),
        )
    })
}

pub async fn serve(config: AppConfig, addr: SocketAddr) -> anyhow::Result<()> {
    let router = app(config).await?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("agent-governance-server listening on http://{addr}");
    axum::serve(listener, router).await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn council_personas(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    Ok(Json(
        json!({ "catalog": persona_catalog_from_pack(&state.council_pack) }),
    ))
}

async fn create_deliberation(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let req: agent_governance_core::CouncilRequest =
        parse_request_json(&body, "invalid council request body")?;
    let run = deliberate_with_pack(req, &state.council_pack).map_err(|err| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_council_request",
            err.to_string(),
        )
    })?;
    let report = render_council_report(&run);
    state
        .councils
        .lock()
        .await
        .insert(run.council_id, run.clone());
    if let Some(db) = &state.db {
        sqlx::query(
            "insert into deliberations (id, json, report_md, created_at) values (?1, ?2, ?3, ?4)",
        )
        .bind(run.council_id.to_string())
        .bind(serde_json::to_string(&run).map_err(internal_error)?)
        .bind(report)
        .bind(run.created_at.to_rfc3339())
        .execute(db)
        .await
        .map_err(internal_error)?;
    }
    Ok(Json(json!({ "deliberation": run })))
}

async fn get_deliberation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    if let Some(run) = state.councils.lock().await.get(&id).cloned() {
        return Ok(Json(json!({ "deliberation": run })));
    }
    if let Some(db) = &state.db {
        if let Some(json_text) = sqlx::query_scalar::<_, String>(
            "select json from deliberations where id = ?1 order by rowid desc limit 1",
        )
        .bind(id.to_string())
        .fetch_optional(db)
        .await
        .map_err(internal_error)?
        {
            let run: agent_governance_core::CouncilRun =
                serde_json::from_str(&json_text).map_err(internal_error)?;
            return Ok(Json(json!({ "deliberation": run })));
        }
    }
    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        format!("unknown deliberation: {id}"),
    ))
}

async fn get_deliberation_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, [(header::HeaderName, &'static str); 1], String), ApiError> {
    require_auth(&state, &headers)?;
    if let Some(run) = state.councils.lock().await.get(&id).cloned() {
        return Ok(markdown(render_council_report(&run)));
    }
    if let Some(db) = &state.db {
        if let Some(report) = sqlx::query_scalar::<_, String>(
            "select report_md from deliberations where id = ?1 order by rowid desc limit 1",
        )
        .bind(id.to_string())
        .fetch_optional(db)
        .await
        .map_err(internal_error)?
        {
            return Ok(markdown(report));
        }
    }
    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        format!("unknown deliberation: {id}"),
    ))
}

async fn create_envelope(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let req: ContextEnvelopeRequest = parse_request_json(&body, "invalid envelope body")?;
    let envelope = create_context_envelope(req, &state.config.context_secret).map_err(|err| {
        ApiError::new(StatusCode::BAD_REQUEST, "invalid_envelope", err.to_string())
    })?;
    persist_json(&state, "context_envelopes", &envelope.id, &envelope).await?;
    Ok(Json(json!({ "envelope": envelope })))
}

#[derive(Deserialize)]
struct VerifyEnvelopeRequest {
    envelope: ContextEnvelope,
}

async fn verify_envelope(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<ContextVerifyResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let req: VerifyEnvelopeRequest = parse_request_json(&body, "invalid envelope verify body")?;
    let response =
        agent_governance_core::verify_context_envelope(&req.envelope, &state.config.context_secret)
            .map_err(internal_error)?;
    Ok(Json(response))
}

async fn create_memory_fact(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let req: agent_governance_core::MemoryFactRequest =
        parse_request_json(&body, "invalid memory fact body")?;
    let record = {
        let mut memory = state.memory.lock().await;
        add_memory_fact(&mut memory, req)
    };
    persist_json(
        &state,
        "memory_events",
        &record.id,
        &json!({ "kind": "fact", "record": record }),
    )
    .await?;
    Ok(Json(json!({ "fact": record })))
}

#[derive(Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}

async fn list_memory_facts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LimitQuery>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let memory = state.memory.lock().await;
    Ok(Json(json!({
        "facts": active_memory_facts(&memory, query.limit.unwrap_or(50), Utc::now()),
        "stale": stale_memory_facts(&memory, Utc::now()),
        "retrieval_role": "untrusted_observation"
    })))
}

async fn create_memory_invalidation(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let req: agent_governance_core::SourceInvalidationRequest =
        parse_request_json(&body, "invalid memory invalidation body")?;
    let record = {
        let mut memory = state.memory.lock().await;
        invalidate_source(&mut memory, req)
    };
    persist_json(
        &state,
        "memory_events",
        &record.id,
        &json!({ "kind": "invalidation", "record": record }),
    )
    .await?;
    Ok(Json(json!({ "invalidation": record })))
}

async fn create_tool_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let req: ToolResultRequest = parse_request_json(&body, "invalid tool result body")?;
    let state_record = {
        let mut tools = state.tools.lock().await;
        record_tool_result(&mut tools, req.clone())
    };
    persist_json(
        &state,
        "tool_events",
        &state_record.name,
        &json!({ "kind": "result", "request": req, "state": state_record }),
    )
    .await?;
    Ok(Json(json!({ "state": state_record })))
}

#[derive(Deserialize)]
struct RankQuery {
    session_type: Option<String>,
}

async fn get_tool_rank(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RankQuery>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let tools = state.tools.lock().await;
    let ranked = rank_tools(&tools, query.session_type.as_deref().unwrap_or("default"));
    let hidden: Vec<_> = ranked.iter().filter(|item| item.hidden).cloned().collect();
    Ok(Json(json!({ "ranked": ranked, "hidden": hidden })))
}

async fn get_tool_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let tools = state.tools.lock().await;
    Ok(Json(json!({ "state": tool_state(&tools, &name) })))
}

async fn create_repair_plan_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let req: agent_governance_core::RepairPlanRequest =
        parse_request_json(&body, "invalid repair plan body")?;
    let plan: RepairPlan = create_repair_plan(req);
    persist_json(&state, "repair_plans", &plan.id, &plan).await?;
    Ok(Json(json!({ "plan": plan })))
}

async fn create_reasoning_trace(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let mut trace: agent_governance_core::ReasoningTrace =
        parse_request_json(&body, "invalid reasoning trace body")?;
    // compute embedding if missing
    if trace.embedding.is_none() {
        let embedding = state
            .embedding_provider
            .embed(&trace.prompt)
            .await
            .map_err(internal_error)?;
        trace.embedding = Some(embedding);
    }
    // persist in DB if available
    if let Some(db) = &state.db {
        let sql =
            "insert or replace into reasoning_traces (id, json, created_at) values (?1, ?2, ?3)";
        sqlx::query(sql)
            .bind(trace.id.to_string())
            .bind(serde_json::to_string(&trace).map_err(internal_error)?)
            .bind(Utc::now().to_rfc3339())
            .execute(db)
            .await
            .map_err(internal_error)?;
    }
    Ok(Json(json!({ "trace": trace })))
}

#[derive(Deserialize)]
struct SearchRequest {
    embedding: Vec<f32>,
    top: Option<usize>,
}

async fn search_reasoning_traces(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let req: SearchRequest = parse_request_json(&body, "invalid search body")?;
    let top = req.top.unwrap_or(5);
    if state.db.is_some() {
        if let Some(database_url) = &state.config.database_url {
            let store = agent_governance_core::SqliteReasoningStore::new(database_url)
                .await
                .map_err(internal_error)?;
            let found = store
                .search_by_embedding(&req.embedding, top)
                .await
                .map_err(internal_error)?;
            let out: Vec<_> = found
                .into_iter()
                .map(|(trace, score)| json!({ "score": score, "trace": trace }))
                .collect();
            return Ok(Json(json!({ "results": out })));
        }
    }
    Ok(Json(json!({ "results": [] })))
}

#[derive(Deserialize)]
struct RecentQuery {
    limit: Option<i64>,
}

async fn list_recent_deliberations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RecentQuery>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let limit = query.limit.unwrap_or(20).clamp(1, 200);
    if let Some(db) = &state.db {
        let rows = sqlx::query("select json from deliberations order by rowid desc limit ?1")
            .bind(limit)
            .fetch_all(db)
            .await
            .map_err(internal_error)?;
        let mut items = Vec::new();
        for row in rows {
            let text: String = row.try_get("json").map_err(internal_error)?;
            let run: agent_governance_core::CouncilRun =
                serde_json::from_str(&text).map_err(internal_error)?;
            items.push(run);
        }
        return Ok(Json(json!({ "deliberations": items })));
    }
    let mut in_mem: Vec<_> = state.councils.lock().await.values().cloned().collect();
    in_mem.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    in_mem.truncate(limit as usize);
    Ok(Json(json!({ "deliberations": in_mem })))
}

async fn list_recent_fanout_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RecentQuery>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let limit = query.limit.unwrap_or(20).clamp(1, 200);
    if let Some(db) = &state.db {
        let rows = sqlx::query("select json from fanout_plans order by rowid desc limit ?1")
            .bind(limit)
            .fetch_all(db)
            .await
            .map_err(internal_error)?;
        let mut items = Vec::new();
        for row in rows {
            let text: String = row.try_get("json").map_err(internal_error)?;
            let run: FanoutRun = serde_json::from_str(&text).map_err(internal_error)?;
            items.push(run);
        }
        return Ok(Json(json!({ "plans": items })));
    }
    let mut in_mem: Vec<_> = state.fanouts.lock().await.values().cloned().collect();
    in_mem.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    in_mem.truncate(limit as usize);
    Ok(Json(json!({ "plans": in_mem })))
}

async fn create_fanout_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let req: agent_governance_core::FanoutRequest =
        parse_request_json(&body, "invalid fanout request body")?;
    let run = planned_fanout(req);
    state
        .fanouts
        .lock()
        .await
        .insert(run.run_id.clone(), run.clone());
    persist_json(&state, "fanout_plans", &run.run_id, &run).await?;
    Ok(Json(json!({ "plan": run })))
}

async fn get_fanout_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    if let Some(run) = state.fanouts.lock().await.get(&id).cloned() {
        return Ok(Json(json!({ "plan": run })));
    }
    let run: FanoutRun = load_json(&state, "fanout_plans", &id).await?;
    Ok(Json(json!({ "plan": run })))
}

async fn get_fanout_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, [(header::HeaderName, &'static str); 1], String), ApiError> {
    require_auth(&state, &headers)?;
    if let Some(run) = state.fanouts.lock().await.get(&id).cloned() {
        return Ok(markdown(render_fanout_report(&run)));
    }
    let run: FanoutRun = load_json(&state, "fanout_plans", &id).await?;
    Ok(markdown(render_fanout_report(&run)))
}

fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if state.config.dev_no_auth {
        return Ok(());
    }
    let expected = state.config.token.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "auth_not_configured",
            "AGENT_GOV_TOKEN is required unless --dev-no-auth is set",
        )
    })?;
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    if constant_time_eq(supplied.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "bearer authentication required",
        ))
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn markdown(body: String) -> (StatusCode, [(header::HeaderName, &'static str); 1], String) {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        body,
    )
}

const TABLE_SCHEMAS: &[&str] = &[
    "create table if not exists deliberations (id text not null, json text not null, report_md text not null, created_at text not null)",
    "create table if not exists context_envelopes (id text not null, json text not null, created_at text not null)",
    "create table if not exists memory_events (id text not null, json text not null, created_at text not null)",
    "create table if not exists tool_events (id text not null, json text not null, created_at text not null)",
    "create table if not exists repair_plans (id text not null, json text not null, created_at text not null)",
    "create table if not exists fanout_plans (id text not null, json text not null, created_at text not null)",
    "create table if not exists reasoning_traces (id text primary key, json text not null, created_at text not null)",
];

async fn init_db(pool: &SqlitePool) -> anyhow::Result<()> {
    for sql in TABLE_SCHEMAS {
        sqlx::query(sql).execute(pool).await?;
    }
    Ok(())
}

async fn persist_json<T: Serialize>(
    state: &AppState,
    table: &'static str,
    id: &str,
    value: &T,
) -> Result<(), ApiError> {
    if let Some(db) = &state.db {
        let sql = format!("insert into {table} (id, json, created_at) values (?1, ?2, ?3)");
        sqlx::query(&sql)
            .bind(id)
            .bind(serde_json::to_string(value).map_err(internal_error)?)
            .bind(Utc::now().to_rfc3339())
            .execute(db)
            .await
            .map_err(internal_error)?;
    }
    Ok(())
}

async fn load_json<T: for<'de> Deserialize<'de>>(
    state: &AppState,
    table: &'static str,
    id: &str,
) -> Result<T, ApiError> {
    let db = state.db.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("unknown id: {id}"),
        )
    })?;
    let sql = format!("select json from {table} where id = ?1 order by rowid desc limit 1");
    let json_text = sqlx::query_scalar::<_, String>(&sql)
        .bind(id)
        .fetch_optional(db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("unknown id: {id}"),
            )
        })?;
    serde_json::from_str(&json_text).map_err(internal_error)
}

fn internal_error(err: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        err.to_string(),
    )
}

async fn hydrate_operational_state_from_db(state: &AppState) -> anyhow::Result<()> {
    let Some(db) = &state.db else {
        return Ok(());
    };

    let memory_rows = sqlx::query("select json from memory_events order by rowid asc")
        .fetch_all(db)
        .await?;
    {
        let mut memory = state.memory.lock().await;
        for row in memory_rows {
            let text: String = row.try_get("json")?;
            let value: Value = serde_json::from_str(&text)?;
            if let Some(kind) = value.get("kind").and_then(|v| v.as_str()) {
                let record = value.get("record").cloned().unwrap_or(Value::Null);
                if kind == "fact" {
                    if let Ok(fact) = serde_json::from_value::<MemoryFactRecord>(record) {
                        memory.facts.push(fact);
                    }
                } else if kind == "invalidation" {
                    if let Ok(invalidation) =
                        serde_json::from_value::<SourceInvalidationRecord>(record)
                    {
                        memory.invalidations.push(invalidation);
                    }
                }
                continue;
            }
            if let Ok(fact) = serde_json::from_value::<MemoryFactRecord>(value.clone()) {
                memory.facts.push(fact);
                continue;
            }
            if let Ok(invalidation) = serde_json::from_value::<SourceInvalidationRecord>(value) {
                memory.invalidations.push(invalidation);
            }
        }
    }

    let tool_rows = sqlx::query("select json from tool_events order by rowid asc")
        .fetch_all(db)
        .await?;
    {
        let mut tools = state.tools.lock().await;
        for row in tool_rows {
            let text: String = row.try_get("json")?;
            let value: Value = serde_json::from_str(&text)?;
            if let Some(request_value) = value.get("request") {
                if let Ok(request) =
                    serde_json::from_value::<ToolResultRequest>(request_value.clone())
                {
                    record_tool_result(&mut tools, request);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod server_tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode as HttpStatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn sqlite_database_url_from_relative_path_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nested").join("fresh.sqlite");

        let url = sqlite_database_url_from_path(&db_path).unwrap();
        assert!(db_path.parent().unwrap().exists());
        assert_eq!(url, format!("sqlite://{}", db_path.as_path().display()));
    }

    #[tokio::test]
    async fn sqlite_database_is_created_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("fresh.sqlite");
        assert!(!db_path.exists());

        let _router = app(AppConfig {
            token: None,
            dev_no_auth: true,
            context_secret: "test-secret".to_string(),
            database_url: Some(sqlite_database_url(&db_path)),
            council_pack_path: None,
            hydrate_state_from_db: false,
            embedding_endpoint: None,
            embedding_api_key: None,
            embedding_dim: 128,
        })
        .await
        .unwrap();

        assert!(db_path.exists());
    }

    #[tokio::test]
    async fn invalid_json_returns_structured_error_body() {
        let router = app(AppConfig {
            token: None,
            dev_no_auth: true,
            context_secret: "test-secret".to_string(),
            database_url: None,
            council_pack_path: None,
            hydrate_state_from_db: false,
            embedding_endpoint: None,
            embedding_api_key: None,
            embedding_dim: 128,
        })
        .await
        .unwrap();

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/council/deliberations")
                    .header("content-type", "application/json")
                    .body(Body::from("{\"invalid\":"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), HttpStatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 64)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["code"], "invalid_json");
    }
}
