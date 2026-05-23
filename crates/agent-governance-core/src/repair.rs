use crate::redaction::redact_text;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairPlanRequest {
    pub failure: String,
    pub target: String,
    #[serde(default)]
    pub allow_production_patch: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RepairPlan {
    pub id: String,
    pub target: String,
    pub classification: String,
    pub sandbox_only: bool,
    pub steps: Vec<String>,
    pub canary: CanaryPlan,
    pub rollback: RollbackPlan,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CanaryPlan {
    pub enabled: bool,
    pub default_percent: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RollbackPlan {
    pub required: bool,
    pub automatic_production_rollback: bool,
}

pub fn create_repair_plan(req: RepairPlanRequest) -> RepairPlan {
    let allow_production = req.allow_production_patch;
    RepairPlan {
        id: format!("repair-{}", Uuid::new_v4().simple()),
        target: req.target,
        classification: classify_failure(&req.failure),
        sandbox_only: !allow_production,
        steps: vec![
            "collect redacted failure evidence".to_string(),
            "draft minimal patch in isolated worktree".to_string(),
            "run focused verifier in sandbox".to_string(),
            "record canary decision".to_string(),
            "prepare rollback command before production rollout".to_string(),
        ],
        canary: CanaryPlan {
            enabled: allow_production,
            default_percent: if allow_production { 5 } else { 0 },
        },
        rollback: RollbackPlan {
            required: true,
            automatic_production_rollback: allow_production,
        },
    }
}

pub fn classify_failure(failure: &str) -> String {
    let text = redact_text(failure).to_ascii_lowercase();
    if text.contains("timeout") || text.contains("timed out") {
        "timeout"
    } else if text.contains("auth") || text.contains("unauthorized") || text.contains("login") {
        "auth"
    } else if text.contains("selector") || text.contains("element") {
        "selector"
    } else if text.contains("policy") || text.contains("forbidden") {
        "policy"
    } else if text.contains("transport") || text.contains("connection") {
        "transport"
    } else {
        "provider"
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_only_by_default() {
        let plan = create_repair_plan(RepairPlanRequest {
            target: "router".to_string(),
            failure: "selector timeout".to_string(),
            allow_production_patch: false,
        });
        assert_eq!(plan.classification, "timeout");
        assert!(plan.sandbox_only);
        assert!(!plan.canary.enabled);
        assert!(plan.rollback.required);
    }
}
