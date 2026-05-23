# End-to-End QA Guide

Use this guide for the release validation pass before publishing `agent-governance-rs` `v0.1.0` on GitHub. It is operator-facing: run each block, compare the expected result, and capture the listed evidence.

## QA Goals

Validate that the release is source-clean, reproducible, documented, secure by default, and usable from a fresh clone. The pass covers local source checks, Rust gates, CLI smoke checks, server/API behavior, SQLite persistence, dynamic pack loading, security behavior, Playwright-assisted GitHub checks, and clean-clone validation.

## Prerequisites

- Rust stable toolchain with `cargo`, `rustfmt`, and `clippy`.
- `gh` authenticated to the target GitHub account.
- `curl`.
- `python3` for small JSON extraction checks.
- Playwright browser access, for example `npx playwright open`, Playwright MCP, or another browser automation runner that can capture screenshots.
- A clean workspace with no generated `target/` directory inside the release tree.
- Expected local repo path:

```bash
cd /home/onceuponaprince/code/borai/ops/borai-cc/agent-governance-rs
```

Expected result: `pwd` prints the `agent-governance-rs` path, and `git status --short` has no unexpected edits for the release copy.

Create an evidence directory for terminal output and browser screenshots:

```bash
export QA_EVIDENCE=/tmp/agent-governance-rs-v0.1.0-qa
mkdir -p "$QA_EVIDENCE"
```

Expected result: the directory exists and is writable.

## Local Source QA

Confirm there is no generated Rust build output in the release tree:

```bash
find . -maxdepth 3 -type d -name target -print
```

Expected result: no output. If `./target` or any nested `target` directory appears, remove it from the release copy before publishing.

Look for obvious tokens, private credentials, and accidental secret strings:

```bash
rg -n "sk-[A-Za-z0-9_-]{8,}|ghp_|github_pat_|Authorization: Bearer [^ ]+|api[_-]?key|password|secret-token-value" . \
  --glob '!Cargo.lock'
```

Expected result: only intentional redaction examples or QA/release guide instructions. Known acceptable matches include the literal `secret-token-value`, `api_key=sk-test-secret-value`, and the regex text in docs or tests. No real provider key, GitHub token, password, private URL, or workspace-specific credential should appear.

Confirm package versions are set for `v0.1.0`:

```bash
rg -n "version =|version.workspace" Cargo.toml crates/*/Cargo.toml
cargo metadata --no-deps --format-version 1 > "$QA_EVIDENCE/cargo-metadata.json"
python3 - <<'PY'
import json
import os
from pathlib import Path
data = json.loads(Path(os.environ["QA_EVIDENCE"], "cargo-metadata.json").read_text())
for package in data["packages"]:
    print(package["name"], package["version"])
PY
```

Expected result: every workspace package intended for the release prints version `0.1.0`.

Confirm crate package contents include packs, docs, examples, and lockfile:

```bash
cargo package -p agent-governance-core --list | tee "$QA_EVIDENCE/package-core.txt"
cargo package -p agent-governance-server --list | tee "$QA_EVIDENCE/package-server.txt"
cargo package -p agent-governance-cli --list | tee "$QA_EVIDENCE/package-cli.txt"
```

Expected result: package lists include the relevant `Cargo.toml` files, source files, README/license metadata, and required pack data. `agent-governance-core` must include `src/default_council.json` because the core crate embeds the default pack with `include_str!`.

Confirm release docs and examples are present:

```bash
test -f README.md
test -f docs/release-guide.md
test -f docs/qa-guide.md
test -f docs/reference-adapters.md
test -f examples/architecture-council/request.json
test -f examples/architecture-council/custom-llms.json
test -f examples/living-research/fanout.json
test -f packs/default/council.json
test -d packs/default-md
```

Expected result: every `test` command exits successfully.

## Rust Gate

Run the required Rust checks from the workspace root:

```bash
cargo fmt --all --check | tee "$QA_EVIDENCE/cargo-fmt.txt"
cargo clippy --workspace --all-targets -- -D warnings | tee "$QA_EVIDENCE/cargo-clippy.txt"
cargo test --workspace --all-targets | tee "$QA_EVIDENCE/cargo-test.txt"
```

Expected result: `cargo fmt` exits `0` with no diff, `cargo clippy` exits `0` with no warnings, and `cargo test` exits `0` with all tests passing.

Failure triage: run `cargo fmt --all` for formatting failures, fix clippy warnings instead of lowering the lint gate, and classify test failures as deterministic, environment-related, or real regressions before publishing.

## CLI Smoke QA

Run the main CLI workflows:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- personas \
  | tee "$QA_EVIDENCE/personas.json"

cargo run -q -p agent-governance-cli --bin agent-governance -- council run \
  --file examples/architecture-council/request.json \
  | tee "$QA_EVIDENCE/council-standard.json"

cargo run -q -p agent-governance-cli --bin agent-governance -- council run \
  --file examples/architecture-council/custom-llms.json \
  | tee "$QA_EVIDENCE/council-custom-llms.json"

cargo run -q -p agent-governance-cli --bin agent-governance -- context sign \
  --file examples/context-envelope/facts.json \
  | tee "$QA_EVIDENCE/context-envelope.json"

cargo run -q -p agent-governance-cli --bin agent-governance -- fanout plan \
  --file examples/living-research/fanout.json \
  | tee "$QA_EVIDENCE/fanout-plan.json"

cargo run -q -p agent-governance-cli --bin living-research -- \
  --prompt "Compare release evidence across sources" \
  | tee "$QA_EVIDENCE/living-research.json"
```

Expected result:

- `personas` prints a JSON catalog with `built_ins`, `triads`, `llms`, and `default_llm`.
- Standard council run returns `status: "awaiting_positions"`, generated member prompts, and resolved member targets.
- Custom LLM council run shows `fast-api` and `browser-reference` member bindings resolved into member `target` values.
- Context signing returns an `envelope` with redacted facts, fact hashes, `instruction_role: "tool_observation"`, and a non-empty `signature`.
- Fanout planning returns `status: "planned"` and result metadata containing `execution: "adapter-driven outside core crate"`. It must not contact browser, CLI, API, or app endpoints.
- `living-research` exits successfully and returns a planned research workflow response.

Negative fanout check:

```bash
python3 - <<'PY'
import json
import os
from pathlib import Path
data = json.loads(Path(os.environ["QA_EVIDENCE"], "fanout-plan.json").read_text())
assert data["status"] == "planned"
for result in data["results"]:
    assert result["status"] == "planned"
    assert result["ok"] is False
    assert result["metadata"]["execution"] == "adapter-driven outside core crate"
print("fanout planning is non-executing")
PY
```

Expected result: prints `fanout planning is non-executing`.

## Server/API QA

Start the server in one terminal:

```bash
AGENT_GOV_TOKEN=dev-token \
cargo run -p agent-governance-cli --bin agent-governance -- server \
  --bind 127.0.0.1:9797
```

Expected result: server logs that it is listening on `http://127.0.0.1:9797`.

In a second terminal, verify health and auth behavior:

```bash
curl -sS http://127.0.0.1:9797/health | tee "$QA_EVIDENCE/health.json"

curl -sS -i http://127.0.0.1:9797/v1/council/personas \
  | tee "$QA_EVIDENCE/auth-rejection.txt"

curl -sS http://127.0.0.1:9797/v1/council/personas \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/api-personas.json"
```

Expected result: `/health` returns `{"status":"ok"}` without auth, unauthenticated `/v1/council/personas` returns HTTP `401` with `authentication_required`, and the authenticated request returns the persona catalog.

Exercise key `/v1/*` routes:

```bash
curl -sS http://127.0.0.1:9797/v1/council/deliberations \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data @examples/architecture-council/request.json \
  | tee "$QA_EVIDENCE/api-deliberation.json"

python3 - <<'PY' > "$QA_EVIDENCE/deliberation-id.txt"
import json
import os
from pathlib import Path
print(json.loads(Path(os.environ["QA_EVIDENCE"], "api-deliberation.json").read_text())["deliberation"]["council_id"])
PY

COUNCIL_ID="$(cat "$QA_EVIDENCE/deliberation-id.txt")"

curl -sS "http://127.0.0.1:9797/v1/council/deliberations/$COUNCIL_ID" \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/api-deliberation-readback.json"

curl -sS "http://127.0.0.1:9797/v1/council/deliberations/$COUNCIL_ID/report.md" \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/api-deliberation-report.md"

curl -sS http://127.0.0.1:9797/v1/context/envelopes \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data @examples/context-envelope/facts.json \
  | tee "$QA_EVIDENCE/api-envelope.json"

python3 - <<'PY' > "$QA_EVIDENCE/api-envelope-verify-body.json"
import json
import os
from pathlib import Path
body = json.loads(Path(os.environ["QA_EVIDENCE"], "api-envelope.json").read_text())
print(json.dumps({"envelope": body["envelope"]}))
PY

curl -sS http://127.0.0.1:9797/v1/context/envelopes/verify \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data @"$QA_EVIDENCE/api-envelope-verify-body.json" \
  | tee "$QA_EVIDENCE/api-envelope-verify.json"

curl -sS http://127.0.0.1:9797/v1/repair/plans \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data '{"failure":"selector timeout in provider UI","target":"browser-bot"}' \
  | tee "$QA_EVIDENCE/api-repair-plan.json"

curl -sS http://127.0.0.1:9797/v1/fanout/plans \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data @examples/living-research/fanout.json \
  | tee "$QA_EVIDENCE/api-fanout-plan.json"
```

Expected result: deliberation create/read/report routes all return the same council ID, report route returns Markdown, envelope verification returns `valid: true`, repair plans return `sandbox_only: true`, and fanout returns `status: "planned"` without external execution.

Stop the server with `Ctrl-C` after this section unless continuing directly into persistence QA.

## Persistence QA

Start the server with a temporary SQLite database:

```bash
export AGENT_GOV_DB="$QA_EVIDENCE/agent-governance.sqlite"
rm -f "$AGENT_GOV_DB"
AGENT_GOV_TOKEN=dev-token \
cargo run -p agent-governance-cli --bin agent-governance -- server \
  --bind 127.0.0.1:8790 \
  --db "$AGENT_GOV_DB"
```

Expected result: server starts on `127.0.0.1:8790`, creates the SQLite database, and keeps running.

In a second terminal:

```bash
cd /home/onceuponaprince/code/borai/ops/borai-cc/agent-governance-rs
export QA_EVIDENCE=/tmp/agent-governance-rs-v0.1.0-qa

curl -sS http://127.0.0.1:8790/v1/council/deliberations \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data @examples/architecture-council/custom-llms.json \
  | tee "$QA_EVIDENCE/persist-deliberation-create.json"

python3 - <<'PY' > "$QA_EVIDENCE/persist-council-id.txt"
import json
import os
from pathlib import Path
print(json.loads(Path(os.environ["QA_EVIDENCE"], "persist-deliberation-create.json").read_text())["deliberation"]["council_id"])
PY

PERSIST_COUNCIL_ID="$(cat "$QA_EVIDENCE/persist-council-id.txt")"

curl -sS "http://127.0.0.1:8790/v1/council/deliberations/$PERSIST_COUNCIL_ID" \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/persist-deliberation-before-restart.json"

curl -sS "http://127.0.0.1:8790/v1/council/deliberations/$PERSIST_COUNCIL_ID/report.md" \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/persist-report-before-restart.md"
```

Expected result: JSON and Markdown readback succeed before restart.

Stop the server with `Ctrl-C`, then restart it with the same `--db` path:

```bash
cd /home/onceuponaprince/code/borai/ops/borai-cc/agent-governance-rs
export QA_EVIDENCE=/tmp/agent-governance-rs-v0.1.0-qa
export AGENT_GOV_DB="$QA_EVIDENCE/agent-governance.sqlite"

AGENT_GOV_TOKEN=dev-token \
cargo run -p agent-governance-cli --bin agent-governance -- server \
  --bind 127.0.0.1:8790 \
  --db "$AGENT_GOV_DB"
```

In a separate terminal, read the same records again:

```bash
cd /home/onceuponaprince/code/borai/ops/borai-cc/agent-governance-rs
export QA_EVIDENCE=/tmp/agent-governance-rs-v0.1.0-qa

PERSIST_COUNCIL_ID="$(cat "$QA_EVIDENCE/persist-council-id.txt")"

curl -sS "http://127.0.0.1:8790/v1/council/deliberations/$PERSIST_COUNCIL_ID" \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/persist-deliberation-after-restart.json"

curl -sS "http://127.0.0.1:8790/v1/council/deliberations/$PERSIST_COUNCIL_ID/report.md" \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/persist-report-after-restart.md"
```

Expected result: both readbacks succeed after restart and contain the same council ID. Stop the server after this check.

## Dynamic Pack QA

Load the default JSON council pack:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- personas \
  --council-pack packs/default/council.json \
  | tee "$QA_EVIDENCE/pack-json-personas.json"
```

Expected result: JSON catalog includes built-in members, domains, LLM profiles, and a `default_llm`.

Load the Markdown pack directory:

```bash
AGENT_GOV_COUNCIL_PACK=packs/default-md \
cargo run -q -p agent-governance-cli --bin agent-governance -- personas \
  | tee "$QA_EVIDENCE/pack-md-personas.json"
```

Expected result: Markdown frontmatter documents resolve into catalog entries and templates.

Confirm custom request LLM profiles resolve into member targets:

```bash
python3 - <<'PY'
import json
import os
from pathlib import Path
data = json.loads(Path(os.environ["QA_EVIDENCE"], "council-custom-llms.json").read_text())
targets = {member["name"]: member.get("target") for member in data["members"]}
assert targets["aristotle"]["name"] == "fast-api"
assert targets["aristotle"]["lane"] == "api"
assert targets["ada"]["name"] == "browser-reference"
assert targets["ada"]["lane"] == "browser"
print("custom LLM profiles resolved")
PY
```

Expected result: prints `custom LLM profiles resolved`.

## Security And Behavior QA

Validate secret redaction in context envelopes:

```bash
python3 - <<'PY'
import json
import os
from pathlib import Path
data = json.loads(Path(os.environ["QA_EVIDENCE"], "context-envelope.json").read_text())
text = json.dumps(data)
assert "secret-token-value" not in text
assert data["envelope"]["signature"]
assert data["envelope"]["fact_hashes"]
print("context secrets redacted and hashes retained")
PY
```

Expected result: prints `context secrets redacted and hashes retained`. The example input intentionally contains `secret-token-value`; it must not appear in the output.

Validate context envelope tamper rejection through the API:

```bash
python3 - <<'PY' > "$QA_EVIDENCE/api-envelope-tampered.json"
import json
import os
from pathlib import Path
body = json.loads(Path(os.environ["QA_EVIDENCE"], "api-envelope.json").read_text())
body["envelope"]["facts"].append("tampered fact")
print(json.dumps({"envelope": body["envelope"]}))
PY

curl -sS http://127.0.0.1:9797/v1/context/envelopes/verify \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data @"$QA_EVIDENCE/api-envelope-tampered.json" \
  | tee "$QA_EVIDENCE/api-envelope-tamper-verify.json"
```

Expected result: response contains `valid: false` and `signature_valid: false`. If the API server from the earlier section is not still running, restart it with `AGENT_GOV_TOKEN=dev-token` on port `9797` before running this check.

Validate sandbox-only repair plan defaults:

```bash
python3 - <<'PY'
import json
import os
from pathlib import Path
data = json.loads(Path(os.environ["QA_EVIDENCE"], "api-repair-plan.json").read_text())
plan = data["plan"]
assert plan["sandbox_only"] is True
assert plan["canary"]["enabled"] is False
assert plan["rollback"]["required"] is True
print("repair plan is sandbox-only by default")
PY
```

Expected result: prints `repair plan is sandbox-only by default`.

Validate tool quarantine and probe recovery:

```bash
curl -sS http://127.0.0.1:9797/v1/tools/results \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data '{"name":"run_bash","ok":false,"latency_ms":50,"cost":0,"risk":0.9,"probe":false,"error":"timeout with Authorization: Bearer secret-token-value"}' \
  > "$QA_EVIDENCE/tool-fail-1.json"

curl -sS http://127.0.0.1:9797/v1/tools/results \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data '{"name":"run_bash","ok":false,"latency_ms":50,"cost":0,"risk":0.9,"probe":false,"error":"timeout"}' \
  > "$QA_EVIDENCE/tool-fail-2.json"

curl -sS http://127.0.0.1:9797/v1/tools/results \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data '{"name":"run_bash","ok":false,"latency_ms":50,"cost":0,"risk":0.9,"probe":false,"error":"timeout"}' \
  | tee "$QA_EVIDENCE/tool-fail-3.json"

curl -sS http://127.0.0.1:9797/v1/tools/rank?session_type=repair \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/tool-rank-quarantined.json"

curl -sS http://127.0.0.1:9797/v1/tools/results \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data '{"name":"run_bash","ok":true,"latency_ms":10,"cost":0,"risk":0,"probe":true,"error":null}' \
  | tee "$QA_EVIDENCE/tool-probe-recovery.json"

curl -sS http://127.0.0.1:9797/v1/tools/run_bash \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/tool-state-after-probe.json"
```

Expected result: after three failures, `tool-rank-quarantined.json` includes `run_bash` in `hidden` with `quarantined: true`; the failure containing `secret-token-value` is redacted; after the successful probe, `tool-state-after-probe.json` shows `quarantined: false` and `consecutive_failures: 0`.

Validate memory invalidation:

```bash
curl -sS http://127.0.0.1:9797/v1/memory/facts \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data '{"fact":"api_key=sk-test-secret-value","source":"doc://stale","ttl_seconds":60,"relevance":1.0}' \
  | tee "$QA_EVIDENCE/memory-fact.json"

curl -sS http://127.0.0.1:9797/v1/memory/facts \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/memory-before-invalidation.json"

curl -sS http://127.0.0.1:9797/v1/memory/invalidations \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data '{"source":"doc://stale","reason":"deleted upstream"}' \
  | tee "$QA_EVIDENCE/memory-invalidation.json"

curl -sS http://127.0.0.1:9797/v1/memory/facts \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/memory-after-invalidation.json"
```

Expected result: before invalidation, the fact appears under `facts` with redacted secret content and `trust_role: "untrusted_observation"`; after invalidation, the same source no longer appears under active `facts`.

## Playwright-Assisted GitHub QA

Use Playwright or browser automation after the repository has been created and pushed. Capture screenshots into `$QA_EVIDENCE`.

Repository home:

```bash
npx playwright screenshot https://github.com/onceuponaprince/agent-governance-rs "$QA_EVIDENCE/github-repo-home.png"
```

Expected result:

- Repository title is `onceuponaprince/agent-governance-rs`.
- Description matches the release guide.
- Visibility is public.
- README renders on the home page.
- License/readme metadata appears as expected.
- Topics include the planned release topics, or the absence is recorded as a settings follow-up.

Browse required files:

```bash
npx playwright screenshot https://github.com/onceuponaprince/agent-governance-rs/blob/main/README.md "$QA_EVIDENCE/github-readme.png"
npx playwright screenshot https://github.com/onceuponaprince/agent-governance-rs/blob/main/docs/release-guide.md "$QA_EVIDENCE/github-release-guide.png"
npx playwright screenshot https://github.com/onceuponaprince/agent-governance-rs/blob/main/docs/qa-guide.md "$QA_EVIDENCE/github-qa-guide.png"
npx playwright screenshot https://github.com/onceuponaprince/agent-governance-rs/tree/main/examples "$QA_EVIDENCE/github-examples.png"
```

Expected result: all pages render with GitHub's Markdown/file UI and no `404`. README links to both the release guide and this QA guide.

Release draft before publishing:

```bash
npx playwright screenshot https://github.com/onceuponaprince/agent-governance-rs/releases/new?tag=v0.1.0 "$QA_EVIDENCE/github-release-draft.png"
```

Expected result: release title is `agent-governance-rs v0.1.0`, release notes match the release guide summary/highlights/disclaimer, and binary artifacts are absent unless a repeatable binary build process was completed.

Published release and tag after publishing:

```bash
npx playwright screenshot https://github.com/onceuponaprince/agent-governance-rs/releases/tag/v0.1.0 "$QA_EVIDENCE/github-release-v0.1.0.png"
npx playwright screenshot https://github.com/onceuponaprince/agent-governance-rs/tree/v0.1.0 "$QA_EVIDENCE/github-tag-v0.1.0.png"
```

Expected result: release page is public and attached to tag `v0.1.0`, source archives are available as GitHub-generated `.zip` and `.tar.gz` downloads, tag page browses the released source, and clone instructions point to `https://github.com/onceuponaprince/agent-governance-rs.git`.

If `npx playwright screenshot` is not available, use the available Playwright MCP/browser session to visit the same URLs and save screenshots with the same filenames.

## Clean-Clone QA

After the release is public, validate from a clean clone in `/tmp`:

```bash
cd /tmp
rm -rf agent-governance-rs-smoke
git clone https://github.com/onceuponaprince/agent-governance-rs.git agent-governance-rs-smoke
cd agent-governance-rs-smoke
git checkout v0.1.0
git rev-parse HEAD | tee "$QA_EVIDENCE/release-commit-sha.txt"
cargo fmt --all --check | tee "$QA_EVIDENCE/clean-clone-fmt.txt"
cargo clippy --workspace --all-targets -- -D warnings | tee "$QA_EVIDENCE/clean-clone-clippy.txt"
cargo test --workspace --all-targets | tee "$QA_EVIDENCE/clean-clone-test.txt"
cargo run -q -p agent-governance-cli --bin agent-governance -- council run \
  --file examples/architecture-council/custom-llms.json \
  | tee "$QA_EVIDENCE/clean-clone-custom-llms.json"
```

Expected result: clone succeeds from the public GitHub URL, checkout points to tag `v0.1.0`, format/clippy/tests pass, and the smoke council command returns `awaiting_positions` with custom LLM target resolution.

## Evidence Checklist

Capture and retain:

- `cargo-fmt.txt`, `cargo-clippy.txt`, and `cargo-test.txt`.
- CLI smoke JSON files for personas, councils, context envelope, fanout plan, and `living-research`.
- API evidence for health, auth rejection, deliberation create/read/report, envelope verify, repair plan, fanout plan, tool quarantine, and memory invalidation.
- Persistence evidence before and after restart.
- Package list files for all three crates.
- GitHub screenshots for repo home, rendered README, rendered release guide, rendered QA guide, examples, release page, and tag page.
- GitHub release URL.
- GitHub tag URL.
- Release commit SHA.
- Pass/fail notes and any deviations from this guide.

## Go/No-Go Checklist

Go if all are true:

- No generated `target/` directory is included.
- Secret scan has only intentional redaction examples and documentation regex matches.
- All package versions are `0.1.0`.
- Required package contents include source, examples, docs, and default pack data.
- `cargo fmt`, `cargo clippy`, and `cargo test` pass locally and in the clean clone.
- CLI smoke commands work.
- `/health` is public, `/v1/*` rejects unauthenticated requests, and authenticated API routes work.
- SQLite persistence survives server restart.
- JSON, Markdown, and custom LLM council packs load correctly.
- Tampered context envelopes are rejected.
- Repair plans are sandbox-only by default.
- Tool quarantine hides failing tools and probe recovery restores them.
- Memory invalidation removes stale source facts from active retrieval.
- Fanout planning remains non-executing in core release checks.
- GitHub release page and tag are public, browsable, and have source archives.

No-go if any are true:

- A real token, private provider URL, private workspace, or private credential appears in source, examples, docs, or release notes.
- `/v1/*` can be accessed without auth unless the server was explicitly started with `--dev-no-auth`.
- Context envelope tampering verifies as valid.
- Repair plans default to production patching.
- Core fanout planning performs network execution.
- Embedded default pack data is missing from crate package contents.
- Clean clone validation fails.
- Release text implies council consensus is truth, formal proof, legal/security advice, production safety, or blockchain consensus.

## Common Failure Triage

- `cargo package` misses pack files: add or correct package include metadata before publishing.
- `curl` returns `503 auth_not_configured`: restart the server with `AGENT_GOV_TOKEN=dev-token` or use `--dev-no-auth` only for explicit local demos, not release QA.
- `curl` returns `401 authentication_required`: verify the `Authorization: Bearer dev-token` header is present and matches the server token.
- SQLite readback fails after restart: confirm the same `--db` path was used and the server was stopped cleanly before restart.
- Playwright screenshots show `404`: confirm the repository was pushed, the branch is `main`, the file exists at the tag or branch, and the repo is public.
- Release source archives are missing: wait for GitHub release page refresh, then reload the published release page.
- Secret scan reports this QA guide: inspect the line. Regex examples and intentionally named redaction fixtures are acceptable; real secrets are not.
