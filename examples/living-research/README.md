This example demonstrates capturing two LLM reasoning traces (a debate) and ingesting them into the JSONL store using the CLI.

Files:
- `trace_a.json` — Trace from agent-alpha (pro-OptionA)
- `trace_b.json` — Trace from agent-bravo (pro-OptionB)
- `ingest.sh` — small script to ingest both traces and list the store

Usage:
```bash
# ingest both traces into `traces_living.jsonl`
./ingest.sh

# list traces
cargo run -p agent-governance-cli -- reasoning list --store traces_living.jsonl --limit 10
```
