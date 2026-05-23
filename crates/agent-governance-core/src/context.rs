use crate::redaction::{redact_text, sha256_text};
use crate::secrets::CONTEXT_ENVELOPE_MAX_TTL_SECS;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Public,
    #[default]
    Internal,
    Sensitive,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContextEnvelopeRequest {
    pub facts: Vec<String>,
    pub source: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default = "default_envelope_ttl_seconds")]
    pub ttl_seconds: i64,
    #[serde(default)]
    pub sensitivity: Sensitivity,
    #[serde(default = "default_provider_boundary")]
    pub provider_boundary: String,
    #[serde(default = "default_trust_role")]
    pub trust_role: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContextEnvelope {
    pub id: String,
    pub version: String,
    pub facts: Vec<String>,
    pub fact_hashes: Vec<String>,
    pub source: String,
    pub source_sha256: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub confidence: f32,
    pub sensitivity: Sensitivity,
    pub provider_boundary: String,
    pub trust_role: String,
    pub instruction_role: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContextVerifyResponse {
    pub valid: bool,
    pub signature_valid: bool,
    pub expired: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("facts must contain between 1 and 32 entries")]
    InvalidFacts,
    #[error("confidence must be between 0.0 and 1.0")]
    InvalidConfidence,
    #[error("ttl_seconds must be between 1 and {}", crate::secrets::CONTEXT_ENVELOPE_MAX_TTL_SECS)]
    InvalidTtl,
    #[error("invalid signing secret length for HMAC")]
    InvalidSigningKey,
    #[error("failed to serialize envelope: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub fn create_context_envelope(
    req: ContextEnvelopeRequest,
    secret: &str,
) -> Result<ContextEnvelope, ContextError> {
    if req.facts.is_empty() || req.facts.len() > 32 {
        return Err(ContextError::InvalidFacts);
    }
    if !(0.0..=1.0).contains(&req.confidence) {
        return Err(ContextError::InvalidConfidence);
    }
    if !(1..=CONTEXT_ENVELOPE_MAX_TTL_SECS).contains(&req.ttl_seconds) {
        return Err(ContextError::InvalidTtl);
    }
    let issued_at = Utc::now();
    let mut envelope = ContextEnvelope {
        id: format!("ctx-{}", Uuid::new_v4().simple()),
        version: "agent-governance-context-envelope-v1".to_string(),
        facts: req.facts.iter().map(|fact| redact_text(fact)).collect(),
        fact_hashes: req.facts.iter().map(|fact| sha256_text(fact)).collect(),
        source: req.source.clone(),
        source_sha256: sha256_text(&req.source),
        issued_at,
        expires_at: issued_at + Duration::seconds(req.ttl_seconds),
        confidence: req.confidence,
        sensitivity: req.sensitivity,
        provider_boundary: req.provider_boundary,
        trust_role: req.trust_role,
        instruction_role: "tool_observation".to_string(),
        signature: String::new(),
    };
    envelope.signature = sign_envelope(&envelope, secret)?;
    Ok(envelope)
}

pub fn verify_context_envelope(
    envelope: &ContextEnvelope,
    secret: &str,
) -> Result<ContextVerifyResponse, ContextError> {
    let expected = sign_envelope(envelope, secret)?;
    let digest_actual = Sha256::digest(envelope.signature.as_bytes());
    let digest_expected = Sha256::digest(expected.as_bytes());
    let signature_valid = bool::from(digest_actual.ct_eq(&digest_expected));
    let expired = envelope.expires_at <= Utc::now();
    Ok(ContextVerifyResponse {
        valid: signature_valid && !expired,
        signature_valid,
        expired,
    })
}

fn sign_envelope(envelope: &ContextEnvelope, secret: &str) -> Result<String, ContextError> {
    let mut body = serde_json::to_value(envelope)?;
    if let Value::Object(map) = &mut body {
        map.remove("signature");
    }
    let canonical = serde_json::to_vec(&sort_json(body))?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| ContextError::InvalidSigningKey)?;
    mac.update(&canonical);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect();
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        other => other,
    }
}

fn default_confidence() -> f32 {
    0.5
}

fn default_envelope_ttl_seconds() -> i64 {
    3600
}

fn default_provider_boundary() -> String {
    "router".to_string()
}

fn default_trust_role() -> String {
    "untrusted_observation".to_string()
}

impl Default for ContextEnvelopeRequest {
    fn default() -> Self {
        Self {
            facts: Vec::new(),
            source: String::new(),
            confidence: default_confidence(),
            ttl_seconds: default_envelope_ttl_seconds(),
            sensitivity: Sensitivity::default(),
            provider_boundary: default_provider_boundary(),
            trust_role: default_trust_role(),
        }
    }
}

pub fn envelope_json(envelope: &ContextEnvelope) -> Value {
    json!({ "envelope": envelope })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_verifies_and_rejects_tamper() {
        let envelope = create_context_envelope(
            ContextEnvelopeRequest {
                facts: vec!["Authorization: Bearer secret-token-value says no".to_string()],
                source: "browser:claude".to_string(),
                confidence: 0.8,
                ttl_seconds: 60,
                sensitivity: Sensitivity::Sensitive,
                provider_boundary: "claude".to_string(),
                trust_role: "untrusted_observation".to_string(),
            },
            "secret",
        )
        .unwrap();
        assert!(!format!("{envelope:?}").contains("secret-token-value"));
        assert!(verify_context_envelope(&envelope, "secret").unwrap().valid);
        let mut tampered = envelope;
        tampered.facts.push("new fact".to_string());
        assert!(!verify_context_envelope(&tampered, "secret").unwrap().valid);
    }
}
