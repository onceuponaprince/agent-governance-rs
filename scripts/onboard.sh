#!/usr/bin/env bash
set -euo pipefail

EVIDENCE_DIR=${1:-${QA_EVIDENCE:-/tmp/agent-governance-rs-v0.1.0-qa}}
SERVER_PORT=${2:-9797}
SERVER_DB=${3:-$EVIDENCE_DIR/agent-governance.sqlite}
mkdir -p "$EVIDENCE_DIR"
export QA_EVIDENCE="$EVIDENCE_DIR"

echo "Starting onboarding smoke checks in: $EVIDENCE_DIR"

# Keep onboarding deterministic and local-only.
cargo run -q -p agent-governance-cli --bin agent-governance -- doctor --json > "$EVIDENCE_DIR/doctor.json"

cargo run -q -p agent-governance-cli --bin agent-governance -- personas | tee "$EVIDENCE_DIR/personas.json"
cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json | tee "$EVIDENCE_DIR/council-standard.json"
cargo run -q -p agent-governance-cli --bin agent-governance -- context sign --file examples/context-envelope/facts.json | tee "$EVIDENCE_DIR/context-envelope.json"
cargo run -q -p agent-governance-cli --bin agent-governance -- fanout plan --file examples/living-research/fanout.json | tee "$EVIDENCE_DIR/fanout-plan.json"

# Optional live API smoke (health + one authenticated endpoint)
export AGENT_GOV_TOKEN=${AGENT_GOV_TOKEN:-dev-token}
export AGENT_GOV_DB="$SERVER_DB"

rm -f "$AGENT_GOV_DB"
(cargo run -q -p agent-governance-cli --bin agent-governance -- server --bind "127.0.0.1:${SERVER_PORT}" --db "$AGENT_GOV_DB") >/tmp/agent-gov-onboard-stdout.log 2>/tmp/agent-gov-onboard-stderr.log &
SERVER_PID=$!

for i in {1..20}; do
  if curl -sS "http://127.0.0.1:${SERVER_PORT}/health" >/dev/null; then
    break
  fi
  sleep 0.5
done

curl -sS "http://127.0.0.1:${SERVER_PORT}/health" | tee "$EVIDENCE_DIR/health.json"
curl -sS -i "http://127.0.0.1:${SERVER_PORT}/v1/council/personas" | tee "$EVIDENCE_DIR/auth-rejection.txt"
curl -sS "http://127.0.0.1:${SERVER_PORT}/v1/council/personas" -H 'Authorization: Bearer dev-token' | tee "$EVIDENCE_DIR/api-personas.json"

kill "$SERVER_PID" || true
wait "$SERVER_PID" 2>/dev/null || true

echo "Onboarding smoke checks complete; logs in /tmp/agent-gov-onboard-*.log"
echo "Evidence: $EVIDENCE_DIR"
