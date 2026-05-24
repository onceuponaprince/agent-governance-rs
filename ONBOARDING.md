Onboarding & Diagnostics
========================


This pass is also available via:

```bash
make onboarding
```

It produces the same key checks and writes artifacts under `$(EVIDENCE_DIR)` by default.

This guide takes you from a prompt to full diagnostics: build, tests, start server, exercise APIs, ingest traces, run vector search, and collect artifacts.

Run these from the repository root.

1) Build, format, lint
```bash
cargo build --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

2) Run tests
```bash
cargo test --workspace
```

3) Start server (dev token, persistent DB)
```bash
RUST_LOG=info AGENT_GOV_TOKEN=dev-token \
  cargo run -p agent-governance-cli --bin agent-governance -- server --bind 127.0.0.1:9797 --db ./agent-governance.sqlite \
  > server.log 2>&1 &
echo $!
```

4) Health & personas smoke checks
```bash
curl -sS http://127.0.0.1:9797/health | jq .

curl -sS http://127.0.0.1:9797/v1/council/personas \
  -H 'Authorization: Bearer dev-token' | jq .
```

5) Create a deliberation (HTTP)
```bash
curl -sS http://127.0.0.1:9797/v1/council/deliberations \
  -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' \
  --data @examples/architecture-council/request.json | jq . > /tmp/api-deliberation.json

COUNCIL_ID=$(jq -r '.deliberation.council_id' /tmp/api-deliberation.json)
echo "created: $COUNCIL_ID"
```

6) Readback JSON & report
```bash
curl -sS "http://127.0.0.1:9797/v1/council/deliberations/$COUNCIL_ID" -H 'Authorization: Bearer dev-token' | jq .

curl -sS "http://127.0.0.1:9797/v1/council/deliberations/$COUNCIL_ID/report.md" -H 'Authorization: Bearer dev-token' | sed -n '1,120p'

# list recent deliberations
curl -sS "http://127.0.0.1:9797/v1/council/deliberations?limit=10" -H 'Authorization: Bearer dev-token' | jq .
```

7) Verify invalid JSON handling (structured error)
```bash
curl -sS http://127.0.0.1:9797/v1/council/deliberations \
  -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' \
  --data '{"invalid": ' | jq .
# Expect: {"error":{"code":"invalid_json","message":"..."}}
```

8) CLI local run
```bash
cargo run -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json | jq . > /tmp/council-run.json
```

9) Reasoning traces — JSONL
```bash
cargo run -p agent-governance-cli --bin agent-governance -- reasoning ingest --file examples/living-research/trace_a.json --store traces.jsonl
cargo run -p agent-governance-cli --bin agent-governance -- reasoning list --store traces.jsonl --limit 50
```

10) Reasoning traces — SQLite + vector search
```bash
cargo run -p agent-governance-cli --bin agent-governance -- reasoning sqlite-ingest --file examples/living-research/trace_a_vec.json --db sqlite:./traces.sqlite
cargo run -p agent-governance-cli --bin agent-governance -- reasoning sqlite-list --db sqlite:./traces.sqlite --limit 50
printf '[0,1,0]' > /tmp/q.json
cargo run -p agent-governance-cli --bin agent-governance -- reasoning sqlite-search-vec --embedding-file /tmp/q.json --db sqlite:./traces.sqlite --top 5
```

11) Council second pass without hand-editing JSON
```bash
cargo run -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json > /tmp/council-run.json

cargo run -p agent-governance-cli --bin agent-governance -- council submit-positions \
  --file /tmp/council-run.json \
  --positions-file examples/architecture-council/positions.json \
  --out /tmp/council-run-second-pass.json

jq . /tmp/council-run-second-pass.json
```

12) Generate request JSON schemas
```bash
cargo run -p agent-governance-cli --bin agent-governance -- schema generate --out-dir schemas
ls -1 schemas/*.json
```

13) Inspect DB & files
```bash
sqlite3 ./traces.sqlite "select count(*) from reasoning_traces;"
sqlite3 ./traces.sqlite "select json from reasoning_traces limit 1;" | jq .
jq -c . traces.jsonl | wc -l
awk 'NF' traces.jsonl | while IFS= read -r line; do echo "$line" | jq . > /dev/null || echo "bad json: $line"; done
```

14) Logs & debug
```bash
tail -n +1 -f server.log
RUST_LOG=debug AGENT_GOV_TOKEN=dev-token cargo run -p agent-governance-cli --bin agent-governance -- server --bind 127.0.0.1:9797 --db ./agent-governance.sqlite > server.log 2>&1 &
```

15) Capture artifacts
```bash
cp /tmp/api-deliberation.json /tmp/evidence/api-deliberation.json
cp traces.sqlite /tmp/evidence/
cp server.log /tmp/evidence/
tar -czf /tmp/evidence.tar.gz -C /tmp evidence
```

Automated tests & next steps
- Run full validation: `cargo test --workspace`.
- Reasoning ingest computes embeddings automatically with deterministic local provider by default.
- To use HTTP embeddings, set `AGENT_GOV_EMBED_ENDPOINT` (and optionally `AGENT_GOV_EMBED_API_KEY`).
- Optional startup hydration from SQLite: set `AGENT_GOV_HYDRATE_STATE=true` for server startup.

Reasoning API (ingest & search)

- Ingest a reasoning trace (server will compute embedding if missing):

```bash
curl -sS http://127.0.0.1:9797/v1/reasoning/traces \
  -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' \
  --data @examples/living-research/trace_a_vec.json | jq .
```

- Search by embedding (POST JSON {"embedding": [...], "top": N}):

```bash
printf '{"embedding": [0,1,0], "top": 3}' > /tmp/search.json
curl -sS http://127.0.0.1:9797/v1/reasoning/search \
  -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' \
  --data @/tmp/search.json | jq .
```

Notes:
- Embeddings are generated by the provider abstraction in `crates/agent-governance-core/src/embedding_provider.rs`.
- Reasoning vector search uses the persistent index in `crates/agent-governance-core/src/persistent_hnsw.rs` with on-disk id mapping and incremental updates.

