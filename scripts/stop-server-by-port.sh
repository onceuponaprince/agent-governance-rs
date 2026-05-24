#!/usr/bin/env bash
set -euo pipefail
# Stop the process listening on a TCP port on localhost.
# Usage: ./scripts/stop-server-by-port.sh 9797

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <port>"; exit 2
fi
PORT="$1"
PID=$(ss -ltnp | rg ":${PORT}\\b" | sed -n 's/.*pid=\([0-9]*\),.*/\1/p' | head -n1 || true)
if [ -z "$PID" ]; then
  echo "no process listening on port ${PORT}"; exit 0
fi
echo "Killing pid $PID on port ${PORT}"
kill -TERM "$PID" || kill -9 "$PID"
echo "killed"
