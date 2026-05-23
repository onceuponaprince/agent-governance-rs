use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

pub fn redact_text(text: &str) -> String {
    let mut out = text.to_string();
    for pattern in secret_patterns() {
        out = pattern
            .replace_all(&out, |caps: &regex::Captures| {
                if caps.name("path").is_some() {
                    "[REDACTED_PATH]".to_string()
                } else if caps.len() >= 3 {
                    let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    if prefix.to_ascii_lowercase().contains("bearer") {
                        format!("{prefix}[REDACTED]")
                    } else {
                        let sep = if prefix.contains(':') { ": " } else { "=" };
                        format!(
                            "{}{sep}[REDACTED]",
                            prefix.trim_end_matches([':', '=', ' '])
                        )
                    }
                } else {
                    "[REDACTED]".to_string()
                }
            })
            .to_string();
    }
    out
}

pub fn redact_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_text(&text)),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_value).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    if is_secret_key(&key) {
                        (key, Value::String("[REDACTED]".to_string()))
                    } else {
                        (key, redact_value(value))
                    }
                })
                .collect(),
        ),
        other => other,
    }
}

pub fn contains_secret(text: &str) -> bool {
    redact_text(text) != text
}

pub fn is_secret_key(key: &str) -> bool {
    static KEY: OnceLock<Regex> = OnceLock::new();
    KEY.get_or_init(|| {
        Regex::new("(?i)(api[_-]?key|token|secret|password|authorization|cookie|credential)")
            .expect("valid secret-key regex")
    })
    .is_match(key)
}

fn secret_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            [
                r"(?i)(api[_-]?key|token|secret|password)\s*[:=]\s*([^\s,;]+)",
                r"(?i)(authorization\s*:\s*bearer\s+)([^\s,;]+)",
                r"\b(sk-[A-Za-z0-9_\-]{8,})\b",
                r"\b(ghp_[A-Za-z0-9_]{8,})\b",
                r"\b(github_pat_[A-Za-z0-9_]{8,})\b",
                r"\b(xox[baprs]-[A-Za-z0-9\-]{8,})\b",
                r"\b(ya29\.[A-Za-z0-9_\-\.]{8,})\b",
                r"\b(AKIA[0-9A-Z]{12,})\b",
                r"(?P<path>(?:/root|/home/[^/\s]+)/(?:\.aws|\.config|\.local/share/borai|\.claude|\.codex|\.gemini|\.docker)[^\s,;:]*)",
            ]
            .into_iter()
            .map(|p| Regex::new(p).expect("valid redaction regex"))
            .collect()
        })
        .as_slice()
}

pub fn sha256_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(text.as_bytes()))
}
