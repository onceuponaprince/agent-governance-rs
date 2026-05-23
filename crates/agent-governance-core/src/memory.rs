use crate::context::Sensitivity;
use crate::redaction::{redact_text, sha256_text};
use crate::secrets::MEMORY_FACT_MAX_TTL_SECS;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

const DEFAULT_DECAY_HALFLIFE_SECONDS: f64 = 3.0 * 24.0 * 60.0 * 60.0;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryFactRequest {
    pub fact: String,
    pub source: String,
    #[serde(default)]
    pub sensitivity: Sensitivity,
    #[serde(default = "default_memory_ttl_seconds")]
    pub ttl_seconds: i64,
    #[serde(default = "default_relevance")]
    pub relevance: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SourceInvalidationRequest {
    pub source: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryFactRecord {
    pub id: String,
    pub fact: String,
    pub fact_sha256: String,
    pub source: String,
    pub source_sha256: String,
    pub sensitivity: Sensitivity,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub relevance: f32,
    pub invalidated: bool,
    pub trust_role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SourceInvalidationRecord {
    pub id: String,
    pub source: String,
    pub source_sha256: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub invalidates_source: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct MemoryStore {
    pub facts: Vec<MemoryFactRecord>,
    pub invalidations: Vec<SourceInvalidationRecord>,
}

pub fn add_memory_fact(store: &mut MemoryStore, req: MemoryFactRequest) -> MemoryFactRecord {
    let now = Utc::now();
    let ttl_secs = req
        .ttl_seconds
        .max(1)
        .min(MEMORY_FACT_MAX_TTL_SECS);
    let record = MemoryFactRecord {
        id: format!("mem-{}", Uuid::new_v4().simple()),
        fact: redact_text(&req.fact),
        fact_sha256: sha256_text(&req.fact),
        source: req.source.clone(),
        source_sha256: sha256_text(&req.source),
        sensitivity: req.sensitivity,
        created_at: now,
        expires_at: now + Duration::seconds(ttl_secs),
        relevance: req.relevance.clamp(0.0, 1.0),
        invalidated: false,
        trust_role: "untrusted_observation".to_string(),
        score: None,
    };
    store.facts.push(record.clone());
    record
}

pub fn invalidate_source(
    store: &mut MemoryStore,
    req: SourceInvalidationRequest,
) -> SourceInvalidationRecord {
    let record = SourceInvalidationRecord {
        id: format!("inv-{}", Uuid::new_v4().simple()),
        source: req.source.clone(),
        source_sha256: sha256_text(&req.source),
        reason: redact_text(&req.reason),
        created_at: Utc::now(),
        invalidates_source: true,
    };
    store.invalidations.push(record.clone());
    record
}

pub fn active_memory_facts(
    store: &MemoryStore,
    limit: usize,
    now: DateTime<Utc>,
) -> Vec<MemoryFactRecord> {
    let invalidated_sources: BTreeSet<_> = store
        .invalidations
        .iter()
        .map(|item| item.source_sha256.as_str())
        .collect();
    let mut facts = Vec::new();
    for record in &store.facts {
        if record.expires_at <= now || invalidated_sources.contains(record.source_sha256.as_str()) {
            continue;
        }
        let age = (now - record.created_at).num_seconds().max(0) as f64;
        let decay = 0.5_f64.powf(age / DEFAULT_DECAY_HALFLIFE_SECONDS);
        let score = ((record.relevance as f64 * decay) * 1_000_000.0).round() / 1_000_000.0;
        if score <= 0.0 {
            continue;
        }
        let mut item = record.clone();
        item.score = Some(score as f32);
        facts.push(item);
    }
    facts.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    facts.truncate(limit.clamp(1, 500));
    facts
}

pub fn stale_memory_facts(store: &MemoryStore, now: DateTime<Utc>) -> Vec<MemoryFactRecord> {
    store
        .facts
        .iter()
        .filter(|record| record.expires_at <= now)
        .cloned()
        .collect()
}

fn default_memory_ttl_seconds() -> i64 {
    7 * 24 * 60 * 60
}

fn default_relevance() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidation_removes_active_facts() {
        let mut store = MemoryStore::default();
        add_memory_fact(
            &mut store,
            MemoryFactRequest {
                fact: "api_key=sk-test-secret-value".to_string(),
                source: "doc://stale".to_string(),
                sensitivity: Sensitivity::Internal,
                ttl_seconds: 60,
                relevance: 1.0,
            },
        );
        assert_eq!(active_memory_facts(&store, 50, Utc::now()).len(), 1);
        invalidate_source(
            &mut store,
            SourceInvalidationRequest {
                source: "doc://stale".to_string(),
                reason: "deleted upstream".to_string(),
            },
        );
        assert!(active_memory_facts(&store, 50, Utc::now()).is_empty());
    }
}
