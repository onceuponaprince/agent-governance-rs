#!/usr/bin/env bash
set -euo pipefail
# Compare council IDs saved before/after restart. Usage:
# ./scripts/run-persistence-check.sh /tmp/agent-governance-rs-v0.1.0-qa

EVIDENCE_DIR=${1:-/tmp/agent-governance-rs-v0.1.0-qa}
BEFORE="$EVIDENCE_DIR/persist-deliberation-before-restart.json"
AFTER="$EVIDENCE_DIR/persist-deliberation-after-restart.json"
OUT="$EVIDENCE_DIR/persist-compare.txt"

if [ ! -f "$BEFORE" ] || [ ! -f "$AFTER" ]; then
  echo "Missing before/after files. Expected: $BEFORE and $AFTER" >&2
  exit 2
fi

before=$(jq -r '.deliberation.council_id // .council_id' "$BEFORE")
after=$(jq -r '.deliberation.council_id // .council_id' "$AFTER")
{
  echo "before=$before"
  echo "after=$after"
  if [ "$before" = "$after" ]; then
    echo "PERSIST_OK"
    exit 0
  else
    echo "PERSIST_MISMATCH"
    exit 3
  fi
} | tee "$OUT"
