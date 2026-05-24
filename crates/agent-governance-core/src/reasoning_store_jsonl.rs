use crate::reasoning::ReasoningTrace;
use anyhow::Context;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

pub struct JsonlStore {
    pub path: PathBuf,
}

impl JsonlStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        JsonlStore { path: path.into() }
    }

    pub fn append(&self, trace: &ReasoningTrace) -> anyhow::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("open {}", self.path.display()))?;
        let mut w = BufWriter::new(file);
        let line = serde_json::to_string(trace)?;
        w.write_all(line.as_bytes())?;
        w.write_all(b"\n")?;
        w.flush()?;
        Ok(())
    }

    pub fn read_all(&self) -> anyhow::Result<Vec<ReasoningTrace>> {
        let file =
            File::open(&self.path).with_context(|| format!("open {}", self.path.display()))?;
        let r = BufReader::new(file);
        let mut out = Vec::new();
        for (i, line) in r.lines().enumerate() {
            let l = line.with_context(|| format!("read line {}", i))?;
            if l.trim().is_empty() {
                continue;
            }
            let t: ReasoningTrace =
                serde_json::from_str(&l).with_context(|| format!("parse line {}", i))?;
            out.push(t);
        }
        Ok(out)
    }
}
