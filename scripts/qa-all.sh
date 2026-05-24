#!/usr/bin/env bash
set -uuo pipefail
# Full QA runner (non-interactive-ish). Captures outputs into an evidence dir.
# Usage: ./scripts/qa-all.sh [/path/to/evidence]

EVIDENCE_DIR=${1:-/tmp/agent-governance-rs-v0.1.0-qa}
mkdir -p "$EVIDENCE_DIR"
echo "QA evidence: $EVIDENCE_DIR"

# ensure helper scripts are executable
chmod +x ./scripts/*.sh || true

pushd "$(pwd)" >/dev/null

echo "== Local source checks =="
# remove build output to keep release copy clean
if [ -d target ]; then
  echo "Removing local ./target to simulate clean release copy"
  rm -rf target || true
fi

rg -n "sk-[A-Za-z0-9_-]{8,}|ghp_|github_pat_|Authorization: Bearer [^ ]+|api[_-]?key|password|secret-token-value" . --glob '!Cargo.lock' > "$EVIDENCE_DIR/secret-scan.txt" 2>/dev/null || true

cargo metadata --no-deps --format-version 1 > "$EVIDENCE_DIR/cargo-metadata.json" 2>&1 || true

echo "== Package lists =="
cargo package -p agent-governance-core --list | tee "$EVIDENCE_DIR/package-core.txt" || true
cargo package -p agent-governance-server --list | tee "$EVIDENCE_DIR/package-server.txt" || true
cargo package -p agent-governance-cli --list | tee "$EVIDENCE_DIR/package-cli.txt" || true

echo "== Rust gate =="
cargo fmt --all --check 2>&1 | tee "$EVIDENCE_DIR/cargo-fmt.txt" || true
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee "$EVIDENCE_DIR/cargo-clippy.txt" || true
cargo test --workspace --all-targets 2>&1 | tee "$EVIDENCE_DIR/cargo-test.txt" || true

echo "== CLI smoke =="
cargo run -q -p agent-governance-cli --bin agent-governance -- personas 2>&1 | tee "$EVIDENCE_DIR/personas.json" || true
cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json 2>&1 | tee "$EVIDENCE_DIR/council-standard.json" || true
cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/custom-llms.json 2>&1 | tee "$EVIDENCE_DIR/council-custom-llms.json" || true
cargo run -q -p agent-governance-cli --bin agent-governance -- context sign --file examples/context-envelope/facts.json 2>&1 | tee "$EVIDENCE_DIR/context-envelope.json" || true
cargo run -q -p agent-governance-cli --bin agent-governance -- fanout plan --file examples/living-research/fanout.json 2>&1 | tee "$EVIDENCE_DIR/fanout-plan.json" || true
cargo run -q -p agent-governance-cli --bin living-research -- --prompt "Compare release evidence across sources" 2>&1 | tee "$EVIDENCE_DIR/living-research.json" || true

echo "== Server/API QA (port 9797) =="
./scripts/start-server.sh --port 9797 --db "$EVIDENCE_DIR/agent-governance.sqlite" --token dev-token --bg || true
for i in {1..20}; do curl -sS http://127.0.0.1:9797/health >/dev/null && break || sleep 0.5; done
curl -sS http://127.0.0.1:9797/health | tee "$EVIDENCE_DIR/health.json" || true
curl -sS -i http://127.0.0.1:9797/v1/council/personas | tee "$EVIDENCE_DIR/auth-rejection.txt" || true
curl -sS http://127.0.0.1:9797/v1/council/personas -H 'Authorization: Bearer dev-token' | tee "$EVIDENCE_DIR/api-personas.json" || true

echo "Creating a deliberation (standard request)"
curl -sS http://127.0.0.1:9797/v1/council/deliberations -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' --data @examples/architecture-council/request.json | tee "$EVIDENCE_DIR/api-deliberation.json" || true
python3 - <<'PY' > "$EVIDENCE_DIR/deliberation-id.txt" 2>/dev/null || true
import json,os
try:
  data=json.load(open(os.path.join(os.environ['EVIDENCE_DIR'],'api-deliberation.json')))
  print(data['deliberation']['council_id'])
except Exception:
  pass
PY

DELIB_ID=$(cat "$EVIDENCE_DIR/deliberation-id.txt" 2>/dev/null || true)
if [ -n "$DELIB_ID" ]; then
  curl -sS "http://127.0.0.1:9797/v1/council/deliberations/$DELIB_ID" -H 'Authorization: Bearer dev-token' | tee "$EVIDENCE_DIR/api-deliberation-readback.json" || true
  curl -sS "http://127.0.0.1:9797/v1/council/deliberations/$DELIB_ID/report.md" -H 'Authorization: Bearer dev-token' | tee "$EVIDENCE_DIR/api-deliberation-report.md" || true
fi

curl -sS http://127.0.0.1:9797/v1/context/envelopes -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' --data @examples/context-envelope/facts.json | tee "$EVIDENCE_DIR/api-envelope.json" || true
python3 - <<'PY' > "$EVIDENCE_DIR/api-envelope-verify-body.json" 2>/dev/null || true
import json,os
body=json.load(open(os.path.join(os.environ['EVIDENCE_DIR'],'api-envelope.json')))
print(json.dumps({'envelope': body['envelope']}))
PY
curl -sS http://127.0.0.1:9797/v1/context/envelopes/verify -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' --data @"$EVIDENCE_DIR/api-envelope-verify-body.json" | tee "$EVIDENCE_DIR/api-envelope-verify.json" || true
curl -sS http://127.0.0.1:9797/v1/repair/plans -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' --data '{"failure":"selector timeout in provider UI","target":"browser-bot"}' | tee "$EVIDENCE_DIR/api-repair-plan.json" || true
curl -sS http://127.0.0.1:9797/v1/fanout/plans -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' --data @examples/living-research/fanout.json | tee "$EVIDENCE_DIR/api-fanout-plan.json" || true

echo "Stopping server on 9797"
./scripts/stop-server-by-port.sh 9797 || true

echo "== Persistence QA (port 8790) =="
./scripts/start-server.sh --port 8790 --db "$EVIDENCE_DIR/agent-governance.sqlite" --token dev-token --bg || true
for i in {1..20}; do curl -sS http://127.0.0.1:8790/health >/dev/null && break || sleep 0.5; done
curl -sS http://127.0.0.1:8790/v1/council/deliberations -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' --data @examples/architecture-council/custom-llms.json | tee "$EVIDENCE_DIR/persist-deliberation-create.json" || true
python3 - <<'PY' > "$EVIDENCE_DIR/persist-council-id.txt" 2>/dev/null || true
import json,os
try:
  data=json.load(open(os.path.join(os.environ['EVIDENCE_DIR'],'persist-deliberation-create.json')))
  print(data['deliberation']['council_id'])
except Exception:
  pass
PY
PERSIST_ID=$(cat "$EVIDENCE_DIR/persist-council-id.txt" 2>/dev/null || true)
if [ -n "$PERSIST_ID" ]; then
  curl -sS "http://127.0.0.1:8790/v1/council/deliberations/$PERSIST_ID" -H 'Authorization: Bearer dev-token' | tee "$EVIDENCE_DIR/persist-deliberation-before-restart.json" || true
  curl -sS "http://127.0.0.1:8790/v1/council/deliberations/$PERSIST_ID/report.md" -H 'Authorization: Bearer dev-token' | tee "$EVIDENCE_DIR/persist-report-before-restart.md" || true
fi

echo "Restarting persistence server to verify readback"
./scripts/stop-server-by-port.sh 8790 || true
sleep 1
./scripts/start-server.sh --port 8790 --db "$EVIDENCE_DIR/agent-governance.sqlite" --token dev-token --bg || true
for i in {1..20}; do curl -sS http://127.0.0.1:8790/health >/dev/null && break || sleep 0.5; done
if [ -n "$PERSIST_ID" ]; then
  curl -sS "http://127.0.0.1:8790/v1/council/deliberations/$PERSIST_ID" -H 'Authorization: Bearer dev-token' | tee "$EVIDENCE_DIR/persist-deliberation-after-restart.json" || true
  curl -sS "http://127.0.0.1:8790/v1/council/deliberations/$PERSIST_ID/report.md" -H 'Authorization: Bearer dev-token' | tee "$EVIDENCE_DIR/persist-report-after-restart.md" || true
fi

echo "Run persistence compare"
./scripts/run-persistence-check.sh "$EVIDENCE_DIR" || true

echo "Stopping persistence server"
./scripts/stop-server-by-port.sh 8790 || true

echo "== Archive evidence =="
./scripts/tar-evidence.sh "$EVIDENCE_DIR" || true

echo "QA run complete. Evidence: $EVIDENCE_DIR"
popd >/dev/null
