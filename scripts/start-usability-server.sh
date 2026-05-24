#!/usr/bin/env bash
set -euo pipefail
# Convenience wrapper to create the usability evidence dir and start the server on the
# usability default port and DB path used in the docs.

QA_EVIDENCE=${QA_EVIDENCE:-/tmp/agent-governance-rs-v0.1.0-usability}
mkdir -p "$QA_EVIDENCE"
DB="$QA_EVIDENCE/usability.sqlite"

echo "Starting usability server (port 9797) with DB: $DB"
exec ./scripts/start-server.sh --port 9797 --db "$DB" --bg
