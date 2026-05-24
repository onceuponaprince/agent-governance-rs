#!/usr/bin/env bash
set -euo pipefail
# Run the short usability checks (health + persona auth checks) and save outputs into the
# usability evidence dir. The server should already be running (see start-usability-server.sh).

QA_EVIDENCE=${1:-/tmp/agent-governance-rs-v0.1.0-usability}
mkdir -p "$QA_EVIDENCE"

echo "Polling /health"
curl -sS http://127.0.0.1:9797/health | tee "$QA_EVIDENCE/health.json"

echo "Unauthenticated persona check (expect 401)"
curl -sS -i http://127.0.0.1:9797/v1/council/personas | tee "$QA_EVIDENCE/auth-rejection.txt"

echo "Authenticated persona catalog"
curl -sS http://127.0.0.1:9797/v1/council/personas -H 'Authorization: Bearer dev-token' | tee "$QA_EVIDENCE/api-personas.json"

echo "Usability checks complete; evidence in $QA_EVIDENCE"
