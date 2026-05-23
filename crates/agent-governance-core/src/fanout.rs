use crate::council::AgentLane;
use crate::redaction::{redact_text, redact_value, sha256_text};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FanoutSource {
    pub lane: AgentLane,
    pub name: String,
    pub endpoint: Option<String>,
    pub provider: Option<String>,
    pub workspace: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub adapter: FanoutAdapter,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FanoutAdapter {
    BrowserBot,
    CliRunnerApi,
    LivingResearch,
    OpenAiCompatible,
    Anthropic,
    #[default]
    CustomHttp,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FanoutRequest {
    pub title: Option<String>,
    pub prompt: String,
    pub sources: Vec<FanoutSource>,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FanoutResult {
    pub lane: AgentLane,
    pub name: String,
    pub ok: bool,
    pub status: String,
    pub text: Option<String>,
    pub latency_ms: u64,
    pub error: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FanoutRun {
    pub run_id: String,
    pub title: Option<String>,
    pub prompt: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub sources: Vec<FanoutSource>,
    pub results: Vec<FanoutResult>,
    pub consensus: FanoutConsensus,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct FanoutConsensus {
    pub usable_count: usize,
    pub failed_count: usize,
    #[serde(default)]
    pub themes: Vec<String>,
    #[serde(default)]
    pub recommended_next_actions: Vec<String>,
}

pub fn planned_fanout(req: FanoutRequest) -> FanoutRun {
    let created_at = Utc::now();
    let prompt_sha256 = sha256_text(&req.prompt);
    let results: Vec<_> = req
        .sources
        .iter()
        .map(|source| FanoutResult {
            lane: source.lane.clone(),
            name: source.name.clone(),
            ok: false,
            status: "planned".to_string(),
            text: None,
            latency_ms: 0,
            error: None,
            metadata: redact_value(serde_json::json!({
                "adapter": source.adapter,
                "endpoint": source.endpoint,
                "provider": source.provider,
                "workspace": source.workspace,
                "model": source.model,
                "prompt_sha256": prompt_sha256,
                "execution": "adapter-driven outside core crate"
            })),
        })
        .collect();
    let consensus = build_fanout_consensus(&results);
    FanoutRun {
        run_id: format!("fanout-{}", Uuid::new_v4().simple()),
        title: req.title,
        prompt: redact_text(&req.prompt),
        status: "planned".to_string(),
        created_at,
        completed_at: None,
        sources: req.sources,
        results,
        consensus,
    }
}

pub fn build_fanout_consensus(results: &[FanoutResult]) -> FanoutConsensus {
    let usable_count = results.iter().filter(|result| result.ok).count();
    let failed_count = results.len().saturating_sub(usable_count);
    let mut actions = Vec::new();
    if usable_count > 0 {
        actions.push(format!(
            "Review {usable_count} usable source response(s) before acting."
        ));
    }
    if failed_count > 0 {
        actions.push(
            "Dispatch planned sources through reference adapters or retry failed lanes."
                .to_string(),
        );
    }
    if results
        .iter()
        .any(|result| result.lane == AgentLane::Browser && !result.ok)
    {
        actions.push(
            "Check browser-bot login state, provider selectors, and stale profiles.".to_string(),
        );
    }
    if results
        .iter()
        .any(|result| result.lane == AgentLane::Cli && !result.ok)
    {
        actions.push(
            "Check CLI-runner API auth, binary availability, and fallback settings.".to_string(),
        );
    }
    if results
        .iter()
        .any(|result| result.lane == AgentLane::App && !result.ok)
    {
        actions.push(
            "Check app adapter availability, including living-research endpoints.".to_string(),
        );
    }
    if actions.is_empty() {
        actions.push("No follow-up required by the deterministic fanout layer.".to_string());
    }
    FanoutConsensus {
        usable_count,
        failed_count,
        themes: vec![],
        recommended_next_actions: actions,
    }
}

pub fn render_fanout_report(run: &FanoutRun) -> String {
    let mut lines = vec![
        format!(
            "# Fanout Report: {}",
            run.title.as_deref().unwrap_or(&run.run_id)
        ),
        String::new(),
        format!("- Run id: `{}`", run.run_id),
        format!("- Status: `{}`", run.status),
        format!("- Created: `{}`", run.created_at),
        format!(
            "- Completed: `{}`",
            run.completed_at.map(|t| t.to_string()).unwrap_or_default()
        ),
        format!("- Sources: `{}`", run.sources.len()),
        format!("- Usable: `{}`", run.consensus.usable_count),
        format!("- Failed: `{}`", run.consensus.failed_count),
        String::new(),
        "## Prompt".to_string(),
        String::new(),
        redact_text(&run.prompt),
        String::new(),
        "## Results".to_string(),
        String::new(),
    ];
    for result in &run.results {
        lines.extend([
            format!("### {:?}: {}", result.lane, result.name),
            String::new(),
            format!("- OK: `{}`", result.ok),
            format!("- Status: `{}`", result.status),
            format!("- Latency: `{}ms`", result.latency_ms),
            String::new(),
        ]);
        if result.ok {
            lines.extend([
                "#### Response".to_string(),
                String::new(),
                redact_text(result.text.as_deref().unwrap_or("")),
                String::new(),
            ]);
        } else if let Some(error) = &result.error {
            lines.extend([
                "#### Failure".to_string(),
                String::new(),
                redact_text(error),
                String::new(),
            ]);
        }
    }
    lines.extend(["## Recommended Next Actions".to_string(), String::new()]);
    lines.extend(
        run.consensus
            .recommended_next_actions
            .iter()
            .map(|item| format!("- {item}")),
    );
    lines.push(String::new());
    lines.join("\n")
}

fn default_timeout_seconds() -> u64 {
    600
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_fanout_models_reference_adapters() {
        let run = planned_fanout(FanoutRequest {
            title: Some("Research".to_string()),
            prompt: "Compare sources".to_string(),
            timeout_seconds: 600,
            sources: vec![
                FanoutSource {
                    lane: AgentLane::Browser,
                    name: "claude-browser".to_string(),
                    endpoint: Some("http://browser-bot:5500".to_string()),
                    provider: Some("claude".to_string()),
                    workspace: Some("default".to_string()),
                    model: None,
                    adapter: FanoutAdapter::BrowserBot,
                },
                FanoutSource {
                    lane: AgentLane::Cli,
                    name: "codex-api-lane".to_string(),
                    endpoint: Some("http://cli-runner:5600/exec".to_string()),
                    provider: Some("codex".to_string()),
                    workspace: None,
                    model: None,
                    adapter: FanoutAdapter::CliRunnerApi,
                },
                FanoutSource {
                    lane: AgentLane::App,
                    name: "living-research".to_string(),
                    endpoint: None,
                    provider: None,
                    workspace: None,
                    model: None,
                    adapter: FanoutAdapter::LivingResearch,
                },
            ],
        });
        assert_eq!(run.sources.len(), 3);
        assert!(render_fanout_report(&run).contains("living-research"));
    }
}
