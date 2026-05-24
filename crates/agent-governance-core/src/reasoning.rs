use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<String>,
    pub agent_id: String,
    pub model: String,
    pub prompt: String,
    pub context_snapshot: Option<serde_json::Value>,
    pub intermediate_steps: Vec<Step>,
    pub decision: Decision,
    pub rationale: Option<String>,
    pub confidence: Option<f32>,
    pub tags: Vec<String>,
    pub provenance: Provenance,
    pub embedding: Option<Vec<f32>>,
    pub metadata: HashMap<String, String>,
}

impl ReasoningTrace {
    pub fn new(
        agent_id: impl Into<String>,
        model: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        ReasoningTrace {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            session_id: None,
            agent_id: agent_id.into(),
            model: model.into(),
            prompt: prompt.into(),
            context_snapshot: None,
            intermediate_steps: Vec::new(),
            decision: Decision::default(),
            rationale: None,
            confidence: None,
            tags: Vec::new(),
            provenance: Provenance::default(),
            embedding: None,
            metadata: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub step_id: Uuid,
    pub step_type: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub score: Option<f32>,
}

impl Step {
    pub fn new(step_type: impl Into<String>, content: impl Into<String>) -> Self {
        Step {
            step_id: Uuid::new_v4(),
            step_type: step_type.into(),
            content: content.into(),
            timestamp: Utc::now(),
            score: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Decision {
    pub action: Option<serde_json::Value>,
    pub chosen_by: Option<String>,
    pub alternatives: Vec<Alternative>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    pub action: serde_json::Value,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Provenance {
    pub source_ids: Vec<Uuid>,
    pub parent_trace: Option<Uuid>,
}
