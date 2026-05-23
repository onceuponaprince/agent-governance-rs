//! Shared limits and insecure dev placeholders. Production configs must refuse the
//! `DEV_*` strings unless `--dev` / [`normalize_dev_mode`] is active.

pub const DEV_CONTEXT_SECRET: &str = "dev-context-secret";
pub const DEV_BEARER_TOKEN: &str = "dev-token";

pub const CONTEXT_ENVELOPE_MAX_TTL_SECS: i64 = 86_400;
pub const MEMORY_FACT_MAX_TTL_SECS: i64 = 90 * 24 * 60 * 60;

pub fn normalize_dev_mode(dev_cli_flag: bool) -> bool {
    dev_cli_flag
        || matches!(
            std::env::var("AGENT_GOV_DEV").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        )
}
