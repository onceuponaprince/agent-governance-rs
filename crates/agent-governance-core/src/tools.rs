use crate::redaction::redact_text;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolResultRequest {
    pub name: String,
    pub ok: bool,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub risk: f64,
    #[serde(default)]
    pub probe: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolEvent {
    pub ts: DateTime<Utc>,
    pub name: String,
    pub ok: bool,
    pub latency_ms: u64,
    pub cost: f64,
    pub risk: f64,
    pub probe: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolState {
    pub name: String,
    pub events: usize,
    pub success_rate: f64,
    pub avg_latency_ms: Option<f64>,
    pub avg_cost: f64,
    pub avg_risk: f64,
    pub consecutive_failures: usize,
    pub quarantined: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RankedTool {
    #[serde(flatten)]
    pub state: ToolState,
    pub score: f64,
    pub hidden: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolStore {
    pub events: Vec<ToolEvent>,
    pub available_tools: BTreeSet<String>,
    pub failure_threshold: usize,
}

impl Default for ToolStore {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            available_tools: [
                "read_file",
                "list_dir",
                "run_bash",
                "browser_ask",
                "cli_exec",
                "api_chat",
                "living_research_query",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            failure_threshold: 3,
        }
    }
}

pub fn record_tool_result(store: &mut ToolStore, req: ToolResultRequest) -> ToolState {
    store.events.push(ToolEvent {
        ts: Utc::now(),
        name: req.name.clone(),
        ok: req.ok,
        latency_ms: req.latency_ms,
        cost: req.cost.max(0.0),
        risk: req.risk.max(0.0),
        probe: req.probe,
        error: req.error.map(|error| redact_text(&error)),
    });
    tool_state(store, &req.name)
}

pub fn tool_state(store: &ToolStore, name: &str) -> ToolState {
    let mut total = 0usize;
    let mut failures = 0usize;
    let mut successes = 0usize;
    let mut latency_sum = 0f64;
    let mut cost_sum = 0f64;
    let mut risk_sum = 0f64;
    let mut quarantined = false;
    for event in store.events.iter().filter(|event| event.name == name) {
        total += 1;
        if event.ok {
            successes += 1;
            failures = 0;
            quarantined = false;
        } else {
            failures += 1;
        }
        latency_sum += event.latency_ms as f64;
        cost_sum += event.cost;
        risk_sum += event.risk;
        if failures >= store.failure_threshold.max(1) {
            quarantined = true;
        }
    }
    let divisor = total.max(1) as f64;
    ToolState {
        name: name.to_string(),
        events: total,
        success_rate: round(successes as f64 / divisor, 4),
        avg_latency_ms: (total > 0).then(|| round(latency_sum / divisor, 2)),
        avg_cost: round(cost_sum / divisor, 6),
        avg_risk: round(risk_sum / divisor, 4),
        consecutive_failures: failures,
        quarantined,
    }
}

pub fn rank_tools(store: &ToolStore, session_type: &str) -> Vec<RankedTool> {
    let mut names = store.available_tools.clone();
    names.extend(store.events.iter().map(|event| event.name.clone()));
    let mut ranked: Vec<_> = names
        .into_iter()
        .map(|name| {
            let state = tool_state(store, &name);
            let latency = state.avg_latency_ms.unwrap_or(500.0);
            let session_boost =
                if session_type == "repair" && matches!(name.as_str(), "read_file" | "list_dir") {
                    -25.0
                } else {
                    0.0
                };
            let score = (if state.quarantined { 1000.0 } else { 0.0 }) - state.success_rate * 100.0
                + latency / 100.0
                + state.avg_cost * 10.0
                + state.avg_risk * 100.0
                + session_boost;
            RankedTool {
                hidden: state.quarantined,
                state,
                score: round(score, 4),
            }
        })
        .collect();
    ranked.sort_by(|a, b| {
        a.score
            .partial_cmp(&b.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.state.name.cmp(&b.state.name))
    });
    ranked
}

fn round(value: f64, digits: i32) -> f64 {
    let factor = 10_f64.powi(digits);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarantine_and_probe_recovery() {
        let mut store = ToolStore {
            failure_threshold: 2,
            ..ToolStore::default()
        };
        for _ in 0..2 {
            record_tool_result(
                &mut store,
                ToolResultRequest {
                    name: "run_bash".to_string(),
                    ok: false,
                    latency_ms: 50,
                    cost: 0.0,
                    risk: 0.9,
                    probe: false,
                    error: None,
                },
            );
        }
        assert!(tool_state(&store, "run_bash").quarantined);
        record_tool_result(
            &mut store,
            ToolResultRequest {
                name: "run_bash".to_string(),
                ok: true,
                latency_ms: 10,
                cost: 0.0,
                risk: 0.0,
                probe: true,
                error: None,
            },
        );
        assert!(!tool_state(&store, "run_bash").quarantined);
    }

    #[test]
    fn success_without_probe_resets_consecutive_failures() {
        let mut store = ToolStore {
            failure_threshold: 2,
            ..ToolStore::default()
        };
        for _ in 0..2 {
            record_tool_result(
                &mut store,
                ToolResultRequest {
                    name: "api_chat".to_string(),
                    ok: false,
                    latency_ms: 10,
                    cost: 0.0,
                    risk: 0.1,
                    probe: false,
                    error: None,
                },
            );
        }
        assert!(tool_state(&store, "api_chat").quarantined);
        record_tool_result(
            &mut store,
            ToolResultRequest {
                name: "api_chat".to_string(),
                ok: true,
                latency_ms: 2,
                cost: 0.0,
                risk: 0.0,
                probe: false,
                error: None,
            },
        );
        assert!(!tool_state(&store, "api_chat").quarantined);
        assert_eq!(tool_state(&store, "api_chat").consecutive_failures, 0);
    }
}
