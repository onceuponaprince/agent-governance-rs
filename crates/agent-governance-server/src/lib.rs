use agent_governance_core::{
    active_memory_facts, add_memory_fact, create_context_envelope, create_repair_plan,
    default_council_pack, deliberate_with_pack, invalidate_source, load_council_pack_from_path,
    persona_catalog_from_pack, planned_fanout, rank_tools, record_tool_result,
    render_council_report, render_fanout_report, stale_memory_facts, tool_state, ContextEnvelope,
    ContextEnvelopeRequest, ContextVerifyResponse, CouncilPack, FanoutRun, MemoryStore, RepairPlan,
    ToolStore,
};
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
    SqlitePool,
};
use std::{
    collections::BTreeMap,
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

#[derive(Clone)]
pub struct AppConfig {
    pub token: Option<String>,
    pub dev_no_auth: bool,
    pub context_secret: String,
    pub database_url: Option<String>,
    pub council_pack_path: Option<PathBuf>,
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
    let state = AppState {
        config,
        db,
        council_pack,
        memory: Arc::new(Mutex::new(MemoryStore::default())),
        tools: Arc::new(Mutex::new(ToolStore::default())),
        councils: Arc::new(Mutex::new(BTreeMap::new())),
        fanouts: Arc::new(Mutex::new(BTreeMap::new())),
    };

    Ok(Router::new()
        .route("/health", get(health))
        .route("/v1/council/personas", get(council_personas))
        .route("/v1/council/deliberations", post(create_deliberation))
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
        .route("/v1/fanout/plans", post(create_fanout_plan))
        .route("/v1/fanout/plans/{id}", get(get_fanout_plan))
        .route("/v1/fanout/plans/{id}/report.md", get(get_fanout_report))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http()))
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
    Json(req): Json<agent_governance_core::CouncilRequest>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
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
    Json(req): Json<ContextEnvelopeRequest>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
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
    Json(req): Json<VerifyEnvelopeRequest>,
) -> Result<Json<ContextVerifyResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let response =
        agent_governance_core::verify_context_envelope(&req.envelope, &state.config.context_secret)
            .map_err(internal_error)?;
    Ok(Json(response))
}

async fn create_memory_fact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<agent_governance_core::MemoryFactRequest>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let record = {
        let mut memory = state.memory.lock().await;
        add_memory_fact(&mut memory, req)
    };
    persist_json(&state, "memory_events", &record.id, &record).await?;
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
    Json(req): Json<agent_governance_core::SourceInvalidationRequest>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let record = {
        let mut memory = state.memory.lock().await;
        invalidate_source(&mut memory, req)
    };
    persist_json(&state, "memory_events", &record.id, &record).await?;
    Ok(Json(json!({ "invalidation": record })))
}

async fn create_tool_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<agent_governance_core::ToolResultRequest>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let state_record = {
        let mut tools = state.tools.lock().await;
        record_tool_result(&mut tools, req)
    };
    persist_json(&state, "tool_events", &state_record.name, &state_record).await?;
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
    Json(req): Json<agent_governance_core::RepairPlanRequest>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
    let plan: RepairPlan = create_repair_plan(req);
    persist_json(&state, "repair_plans", &plan.id, &plan).await?;
    Ok(Json(json!({ "plan": plan })))
}

async fn create_fanout_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<agent_governance_core::FanoutRequest>,
) -> Result<Json<Value>, ApiError> {
    require_auth(&state, &headers)?;
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

#[cfg(test)]
mod server_tests {
    use super::*;

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
        })
        .await
        .unwrap();

        assert!(db_path.exists());
    }
}
