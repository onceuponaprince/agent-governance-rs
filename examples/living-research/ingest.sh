#!/usr/bin/env bash
set -euo pipefail

STORE=traces_living.jsonl

echo "Ingesting trace_a.json"
cargo run -p agent-governance-cli --bin agent-governance -- reasoning ingest --file examples/living-research/trace_a.json --store $STORE
echo "Ingesting trace_b.json"
cargo run -p agent-governance-cli --bin agent-governance -- reasoning ingest --file examples/living-research/trace_b.json --store $STORE

echo
echo "Listing traces in $STORE"
cargo run -p agent-governance-cli --bin agent-governance -- reasoning list --store $STORE --limit 10

echo
echo "Done"
