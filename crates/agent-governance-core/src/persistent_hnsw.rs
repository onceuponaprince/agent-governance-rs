use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexItem {
    id: Uuid,
    embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct IndexData {
    dim: usize,
    items: Vec<IndexItem>,
}

/// Persistent ANN index used by reasoning search.
///
/// Notes:
/// - This keeps stable id<->vector mapping on disk.
/// - Search path is deterministic cosine over persisted items; this is a compatibility
///   layer that can be swapped to native HNSW internals behind the same API.
pub struct PersistentHnswIndex {
    path: PathBuf,
    data: IndexData,
}

impl PersistentHnswIndex {
    pub fn load_or_create(path: impl AsRef<Path>, dim: usize) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("read index file {}", path.display()))?;
            let mut data: IndexData = serde_json::from_str(&raw)
                .with_context(|| format!("parse index file {}", path.display()))?;
            if data.dim == 0 {
                data.dim = dim;
            }
            Ok(Self { path, data })
        } else {
            let data = IndexData {
                dim,
                items: Vec::new(),
            };
            let index = Self { path, data };
            index.save()?;
            Ok(index)
        }
    }

    pub fn upsert(&mut self, id: Uuid, embedding: Vec<f32>) -> anyhow::Result<()> {
        if self.data.items.is_empty() {
            self.data.dim = embedding.len();
        }
        if embedding.len() != self.data.dim {
            return Ok(());
        }
        if let Some(existing) = self.data.items.iter_mut().find(|item| item.id == id) {
            existing.embedding = embedding;
        } else {
            self.data.items.push(IndexItem { id, embedding });
        }
        self.save()
    }

    pub fn remove(&mut self, id: Uuid) -> anyhow::Result<()> {
        self.data.items.retain(|item| item.id != id);
        self.save()
    }

    pub fn rebuild(&mut self, dim: usize, items: Vec<(Uuid, Vec<f32>)>) -> anyhow::Result<()> {
        self.data.dim = dim;
        self.data.items = items
            .into_iter()
            .filter(|(_, emb)| emb.len() == dim)
            .map(|(id, embedding)| IndexItem { id, embedding })
            .collect();
        self.save()
    }

    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(Uuid, f32)> {
        if query.len() != self.data.dim {
            return Vec::new();
        }
        let norm_q: f32 = query.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm_q == 0.0 {
            return Vec::new();
        }
        let mut scored: Vec<(Uuid, f32)> = self
            .data
            .items
            .iter()
            .filter_map(|item| {
                let dot: f32 = item
                    .embedding
                    .iter()
                    .zip(query.iter())
                    .map(|(a, b)| a * b)
                    .sum();
                let norm_v: f32 = item.embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
                if norm_v == 0.0 {
                    return None;
                }
                Some((item.id, dot / (norm_q * norm_v)))
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    fn save(&self) -> anyhow::Result<()> {
        let raw = serde_json::to_string_pretty(&self.data)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create index dir {}", parent.display()))?;
        }
        fs::write(&self.path, raw)
            .with_context(|| format!("write index file {}", self.path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_index_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.hnsw.json");
        let id = Uuid::new_v4();

        let mut idx = PersistentHnswIndex::load_or_create(&path, 3).unwrap();
        idx.upsert(id, vec![0.0, 1.0, 0.0]).unwrap();

        let idx2 = PersistentHnswIndex::load_or_create(&path, 3).unwrap();
        let found = idx2.search(&[0.0, 1.0, 0.0], 1);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, id);
    }
}
