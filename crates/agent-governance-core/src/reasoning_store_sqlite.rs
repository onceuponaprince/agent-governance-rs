use crate::persistent_hnsw::PersistentHnswIndex;
use crate::reasoning::ReasoningTrace;
use anyhow::Context;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct SqliteReasoningStore {
    pool: SqlitePool,
    index: Mutex<PersistentHnswIndex>,
}

impl SqliteReasoningStore {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        // Accept either `sqlite:./path` or `sqlite://./path` and create file if missing
        let url = if database_url.starts_with("sqlite://") {
            database_url.to_string()
        } else if let Some(stripped_path) = database_url.strip_prefix("sqlite:") {
            format!("sqlite://{}", stripped_path)
        } else {
            format!("sqlite://{}", database_url)
        };
        let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .with_context(|| format!("connect {}", url))?;
        let index_path = index_path_for_db_url(&url);
        let index = PersistentHnswIndex::load_or_create(index_path, 128)?;
        let s = Self {
            pool,
            index: Mutex::new(index),
        };
        s.init().await?;
        s.rebuild_index_from_db().await?;
        Ok(s)
    }

    async fn init(&self) -> anyhow::Result<()> {
        sqlx::query(
            "create table if not exists reasoning_traces (id text primary key, json text not null, created_at text not null)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert(&self, trace: &ReasoningTrace) -> anyhow::Result<()> {
        let sql =
            "insert or replace into reasoning_traces (id, json, created_at) values (?1, ?2, ?3)";
        sqlx::query(sql)
            .bind(trace.id.to_string())
            .bind(serde_json::to_string(trace)?)
            .bind(trace.timestamp.to_rfc3339())
            .execute(&self.pool)
            .await?;
        if let Some(ref emb) = trace.embedding {
            let mut idx = self.index.lock().await;
            idx.upsert(trace.id, emb.clone())?;
        }
        Ok(())
    }

    pub async fn remove(&self, id: Uuid) -> anyhow::Result<()> {
        let sql = "delete from reasoning_traces where id = ?1";
        sqlx::query(sql)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        let mut idx = self.index.lock().await;
        idx.remove(id)?;
        Ok(())
    }

    pub async fn list(&self, limit: i64) -> anyhow::Result<Vec<ReasoningTrace>> {
        let rows = sqlx::query("select json from reasoning_traces order by rowid desc limit ?1")
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            let text: String = r.try_get("json")?;
            let t: ReasoningTrace = serde_json::from_str(&text)?;
            out.push(t);
        }
        Ok(out)
    }

    pub async fn query_by_tag(&self, tag: &str, limit: i64) -> anyhow::Result<Vec<ReasoningTrace>> {
        // crude JSON text match for prototype
        let pattern = format!("%\"{}\"%", tag);
        let sql =
            "select json from reasoning_traces where json like ?1 order by rowid desc limit ?2";
        let rows = sqlx::query(sql)
            .bind(pattern)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            let text: String = r.try_get("json")?;
            let t: ReasoningTrace = serde_json::from_str(&text)?;
            out.push(t);
        }
        Ok(out)
    }

    pub async fn query_by_time_range(
        &self,
        since: &str,
        until: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<ReasoningTrace>> {
        let sql = "select json from reasoning_traces where created_at >= ?1 and created_at <= ?2 order by rowid desc limit ?3";
        let rows = sqlx::query(sql)
            .bind(since)
            .bind(until)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            let text: String = r.try_get("json")?;
            let t: ReasoningTrace = serde_json::from_str(&text)?;
            out.push(t);
        }
        Ok(out)
    }

    pub async fn search_by_embedding(
        &self,
        query: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<(ReasoningTrace, f32)>> {
        let found = {
            let idx = self.index.lock().await;
            idx.search(query, top_k)
        };
        // map back to full traces
        let mut out: Vec<(ReasoningTrace, f32)> = Vec::new();
        for (id, score) in found {
            if let Some(t) = self.get(id).await? {
                out.push((t, score));
            }
        }
        Ok(out)
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<ReasoningTrace>> {
        let row = sqlx::query(
            "select json from reasoning_traces where id = ?1 order by rowid desc limit 1",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        if let Some(r) = row {
            let text: String = r.try_get("json")?;
            let t: ReasoningTrace = serde_json::from_str(&text)?;
            Ok(Some(t))
        } else {
            Ok(None)
        }
    }

    async fn rebuild_index_from_db(&self) -> anyhow::Result<()> {
        let rows = sqlx::query("select json from reasoning_traces order by rowid desc")
            .fetch_all(&self.pool)
            .await?;
        let mut items: Vec<(Uuid, Vec<f32>)> = Vec::new();
        let mut dim = 128usize;
        for r in rows {
            let text: String = r.try_get("json")?;
            let t: ReasoningTrace = serde_json::from_str(&text)?;
            if let Some(emb) = t.embedding {
                dim = emb.len();
                items.push((t.id, emb));
            }
        }
        let mut idx = self.index.lock().await;
        idx.rebuild(dim, items)?;
        Ok(())
    }
}

fn index_path_for_db_url(url: &str) -> PathBuf {
    let clean = if let Some(rest) = url.strip_prefix("sqlite://") {
        rest
    } else if let Some(rest) = url.strip_prefix("sqlite:") {
        rest
    } else {
        url
    };
    PathBuf::from(format!("{}.hnsw.json", clean))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_roundtrip_and_vector_ranking() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("traces.sqlite");
        let db_url = format!("sqlite:{}", db_path.display());
        let store = SqliteReasoningStore::new(&db_url).await.unwrap();

        let mut a = ReasoningTrace::new("a", "m", "trace a");
        a.embedding = Some(vec![0.0, 1.0, 0.0]);
        let mut b = ReasoningTrace::new("b", "m", "trace b");
        b.embedding = Some(vec![1.0, 0.0, 0.0]);

        store.insert(&a).await.unwrap();
        store.insert(&b).await.unwrap();

        let listed = store.list(10).await.unwrap();
        assert_eq!(listed.len(), 2);

        let found = store
            .search_by_embedding(&[0.0, 1.0, 0.0], 2)
            .await
            .unwrap();
        assert!(!found.is_empty());
        assert_eq!(found[0].0.id, a.id);
    }
}
