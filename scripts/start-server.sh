#!/usr/bin/env bash
set -euo pipefail
# Start the agent-governance server with sensible defaults and env var overrides.
# Usage: ./scripts/start-server.sh [--port PORT] [--db PATH] [--token TOKEN] [--bg]
# Examples:
#   ./scripts/start-server.sh --port 9797 --db ./agent-governance.sqlite
#   ./scripts/start-server.sh --db /tmp/agent-governance.sqlite --bg

PORT=9797
DB=""
TOKEN="dev-token"
BG=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="$2"; shift 2;;
    --db) DB="$2"; shift 2;;
    --token) TOKEN="$2"; shift 2;;
    --bg) BG=1; shift 1;;
    -h|--help) echo "Usage: $0 [--port PORT] [--db PATH] [--token TOKEN] [--bg]"; exit 0;;
    *) echo "Unknown arg: $1"; exit 1;;
  esac
done

if [ -z "$DB" ]; then
  echo "Error: --db PATH is required (where the server will create the sqlite DB)" >&2
  exit 2
fi

export AGENT_GOV_DB="$DB"
export AGENT_GOV_TOKEN="$TOKEN"

CMD=(cargo run -p agent-governance-cli --bin agent-governance -- server --bind 127.0.0.1:${PORT} --db "$AGENT_GOV_DB")

echo "Starting server on 127.0.0.1:${PORT} using DB: $AGENT_GOV_DB"
if [ "$BG" -eq 1 ]; then
  "${CMD[@]}" &
  disown
  echo "Server started in background (check logs in terminal where you launched it)."
else
  exec "${CMD[@]}"
fi
