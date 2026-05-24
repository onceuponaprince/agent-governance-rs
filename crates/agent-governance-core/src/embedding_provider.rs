use crate::embedding::compute_embedding;
use anyhow::{bail, Context};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, input: &str) -> anyhow::Result<Vec<f32>>;
    fn dimension(&self) -> usize;
}

#[derive(Clone, Debug)]
pub struct DeterministicEmbeddingProvider {
    dim: usize,
}

impl DeterministicEmbeddingProvider {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

#[async_trait]
impl EmbeddingProvider for DeterministicEmbeddingProvider {
    async fn embed(&self, input: &str) -> anyhow::Result<Vec<f32>> {
        Ok(compute_embedding(input, self.dim))
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

#[derive(Clone, Debug)]
pub struct HttpEmbeddingProvider {
    endpoint: String,
    api_key: Option<String>,
    dim: usize,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    input: &'a str,
}

#[derive(Debug, Deserialize)]
struct EmbedResponseSimple {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct EmbedResponseDataItem {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct EmbedResponseData {
    data: Vec<EmbedResponseDataItem>,
}

impl HttpEmbeddingProvider {
    pub fn new(endpoint: impl Into<String>, api_key: Option<String>, dim: usize) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key,
            dim,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for HttpEmbeddingProvider {
    async fn embed(&self, input: &str) -> anyhow::Result<Vec<f32>> {
        let mut req = self
            .client
            .post(&self.endpoint)
            .json(&EmbedRequest { input });
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let response = req.send().await.context("embedding request failed")?;
        if !response.status().is_success() {
            bail!("embedding endpoint returned status {}", response.status());
        }
        let body = response
            .text()
            .await
            .context("failed to read embedding response body")?;

        if let Ok(simple) = serde_json::from_str::<EmbedResponseSimple>(&body) {
            if simple.embedding.len() != self.dim {
                bail!(
                    "embedding dimension mismatch: expected {}, got {}",
                    self.dim,
                    simple.embedding.len()
                );
            }
            return Ok(simple.embedding);
        }

        if let Ok(data) = serde_json::from_str::<EmbedResponseData>(&body) {
            if let Some(first) = data.data.first() {
                if first.embedding.len() != self.dim {
                    bail!(
                        "embedding dimension mismatch: expected {}, got {}",
                        self.dim,
                        first.embedding.len()
                    );
                }
                return Ok(first.embedding.clone());
            }
        }

        bail!("unrecognized embedding response shape")
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockEmbeddingProvider {
        dim: usize,
        value: f32,
    }

    #[async_trait]
    impl EmbeddingProvider for MockEmbeddingProvider {
        async fn embed(&self, _input: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![self.value; self.dim])
        }

        fn dimension(&self) -> usize {
            self.dim
        }
    }

    #[tokio::test]
    async fn deterministic_provider_returns_fixed_dimension() {
        let p = DeterministicEmbeddingProvider::new(16);
        let emb = p.embed("hello").await.unwrap();
        assert_eq!(emb.len(), 16);
    }

    #[tokio::test]
    async fn mock_provider_can_be_used_as_trait_object() {
        let p: Box<dyn EmbeddingProvider> = Box::new(MockEmbeddingProvider {
            dim: 8,
            value: 0.25,
        });
        let emb = p.embed("ignored").await.unwrap();
        assert_eq!(emb, vec![0.25; 8]);
    }
}
