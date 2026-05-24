#!/usr/bin/env bash
set -euo pipefail
# Create a tar.gz of an evidence directory.
# Usage: ./scripts/tar-evidence.sh /tmp/agent-governance-rs-v0.1.0-qa

EVIDENCE_DIR=${1:-/tmp/agent-governance-rs-v0.1.0-qa}
if [ ! -d "$EVIDENCE_DIR" ]; then
  echo "Evidence dir not found: $EVIDENCE_DIR" >&2; exit 2
fi
OUT="${EVIDENCE_DIR}.tar.gz"
echo "Creating $OUT"
tar -czf "$OUT" -C "$(dirname "$EVIDENCE_DIR")" "$(basename "$EVIDENCE_DIR")"
echo "Created $OUT"
