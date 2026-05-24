# agent-governance-rs

Experimental Rust toolkit for agent councils, context hygiene, and auditable decision workflows.

Originally extracted from Borai's agent infrastructure, this repo keeps Borai, browser-bot, CLI-runner, and living-research as reference integrations rather than product assumptions. The core crate does not execute external agents; it builds governance records, prompt rounds, context envelopes, memory hygiene state, tool quarantine rankings, repair plans, and adapter-neutral fanout plans.
 
## Moe — plain English

What this does (simple): let a small team or a program create a "council" of members (people or agent roles), generate the prompts and records they would use to deliberate, and store those deliberations as auditable, tamper-evident records. It helps you plan who to ask, gather the responses later, sign and verify context, and produce reproducible reports — without requiring any cloud LLM or network execution by default.

Why now?: agent-driven workflows are spreading quickly, but teams lack lightweight, reproducible governance primitives. This project is a local-first, adapter-driven attempt to separate planning from execution so you can prototype, audit, and share deliberations safely and repeatably.

Who I am and what I want to achieve: I'm Moe — building tools so teams can experiment with agent councils and governance patterns without vendor lock-in or hidden side-effects. The short goal: make it easy to capture, verify, and reproduce decision workflows so humans and tools can reason about them clearly.

## Packages

- `agent-governance-core`: pure Rust types and logic. Council members, domains, and prompt templates are loaded from JSON or Markdown frontmatter packs.
- `agent-governance-server`: Axum server exposing stable `/v1/*` APIs. Bearer auth is required by default with `AGENT_GOV_TOKEN`. Secure defaults refuse `dev-token` / `dev-context-secret` unless you pass `--dev` or set `AGENT_GOV_DEV=1`; `--dev-no-auth` only works together with dev mode.
- `agent-governance-cli`: app package with `agent-governance` plus the `living-research` binary alias.

## Dynamic Council Packs

No agents, domains, triads, or prompt-round instructions are hard-coded in Rust. Load them from a council pack:

```bash
agent-governance personas --council-pack ./packs/default/council.json
agent-governance council run --file examples/architecture-council/request.json --council-pack ./packs/default/council.json
AGENT_GOV_COUNCIL_PACK=./packs/default-md agent-governance personas
```

A pack may be one JSON file, one Markdown file with YAML frontmatter, or a directory containing `.json` and `.md` documents. Markdown documents use `kind: member`, `kind: domain`, or `kind: template`; template bodies are Markdown and may use placeholders such as `{{problem}}`, `{{persona_profile}}`, and `{{member_summary}}`.

LLM profiles are dynamic too. Define `llms[]` in the pack or request and bind them with `default_llm` or per-member `llm`:

```json
{
  "llms": [
    {
      "name": "fast-api",
      "lane": "api",
      "provider": "openrouter",
      "model": "example/fast-reviewer",
      "endpoint": "https://openrouter.ai/api/v1",
      "temperature": 0.1,
      "max_tokens": 2048
    }
  ],
  "default_llm": "fast-api",
  "members": [{ "name": "aristotle", "llm": "fast-api" }]
}
```

Resolution order is explicit member target, explicit member LLM, member pack target, request `targets[]`, then request or pack `default_llm`. This lets callers override pack defaults without editing the pack.

### Create A Custom Council Pack From Scratch

1. Create a directory and minimal pack file:

```bash
mkdir -p packs/my-pack
cat > packs/my-pack/council.json <<'JSON'
{
  "members": [
    {
      "name": "builder",
      "persona": "pragmatic implementer",
      "domain": "architecture",
      "polarity": "pro",
      "expertise": ["delivery", "ops"],
      "principles": ["prefer reversible changes"],
      "style": "concise",
      "stance": "ship safe increments",
      "constraints": []
    },
    {
      "name": "critic",
      "persona": "failure-focused reviewer",
      "domain": "architecture",
      "polarity": "con",
      "expertise": ["risk", "testing"],
      "principles": ["surface hidden assumptions"],
      "style": "direct",
      "stance": "prove before rollout",
      "constraints": []
    }
  ],
  "domains": [
    {
      "name": "architecture",
      "members": ["builder", "critic"]
    }
  ],
  "prompt_templates": [
    {
      "name": "member-default",
      "round": "deliberation",
      "scope": "member",
      "modes": ["quick", "duo", "full"],
      "template": "Problem: {{problem}}\\nRole: {{member_summary}}\\nProvide a position with vote and rationale."
    }
  ]
}
JSON
```

2. Validate the pack is loadable:

```bash
cargo run -p agent-governance-cli --bin agent-governance -- personas --council-pack ./packs/my-pack/council.json
```

3. Run a request with your custom pack:

```bash
cargo run -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json --council-pack ./packs/my-pack/council.json
```

## Quickstart

From the repository root, **`agent-governance doctor`** prints the CLI version, copy-paste starters, and paths under `examples/` and `packs/`. For raw HTTP, use [docs/api-cheatsheet.md](docs/api-cheatsheet.md).

**Fastest loop (CLI only, no server):**

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- personas
cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json
```

**Local HTTP server:** copy [.env.example](.env.example) and set `AGENT_GOV_DEV=1` plus `AGENT_GOV_TOKEN=dev-token` if you follow the README commands, then:

```bash
set -a && source .env && set +a   # bash/zsh: load .env.example-derived file
cargo run -p agent-governance-cli --bin agent-governance -- server --bind 127.0.0.1:9797 --dev --db ./agent-governance.sqlite
```

Logs print the base URL, `/health`, and whether Bearer auth applies. `GET /health` stays unauthenticated and includes build `version`, `api_auth`, and persistence flags.

**More examples:**

```bash
cargo run -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/custom-llms.json
cargo run -p agent-governance-cli --bin agent-governance -- context sign --file examples/context-envelope/facts.json --dev
cargo run -p agent-governance-cli --bin agent-governance -- fanout plan --file examples/living-research/fanout.json
```

Onboarding & diagnostics: see [ONBOARDING.md](ONBOARDING.md) for a step-by-step walkthrough from prompt to full diagnostics.

Server APIs include:

- `GET /v1/council/personas`
- `POST /v1/council/deliberations`
- `GET /v1/council/deliberations?limit=20`
- `GET /v1/council/deliberations/{id}`
- `GET /v1/council/deliberations/{id}/report.md`
- `POST /v1/context/envelopes`
- `POST /v1/context/envelopes/verify`
- `POST /v1/memory/facts`
- `GET /v1/memory/facts`
- `POST /v1/memory/invalidations`
- `POST /v1/tools/results`
- `GET /v1/tools/rank`
- `GET /v1/tools/{name}`
- `POST /v1/repair/plans`
- `POST /v1/fanout/plans`
- `GET /v1/fanout/plans?limit=20`
- `GET /v1/fanout/plans/{id}`
- `GET /v1/fanout/plans/{id}/report.md`
- `POST /v1/reasoning/traces`
- `POST /v1/reasoning/search`

### Council Submit Positions (Second Pass)

Use `submit-positions` to avoid hand-editing full council JSON for the second pass.

From a run JSON file:

```bash
cargo run -p agent-governance-cli --bin agent-governance -- council submit-positions \
  --file /tmp/council-run.json \
  --positions-file examples/architecture-council/positions.json \
  --out /tmp/council-run-second-pass.json
```

From SQLite by deliberation id:

```bash
cargo run -p agent-governance-cli --bin agent-governance -- council submit-positions \
  --db sqlite:./agent-governance.sqlite \
  --id 06452d25-a7ce-4fed-9277-b1e7c05cbd31 \
  --positions-file examples/architecture-council/positions.json \
  --commit
```

### JSON Schema Files For /v1 Request Bodies

Generate schemas:

```bash
cargo run -p agent-governance-cli --bin agent-governance -- schema generate --out-dir schemas
```

Generated files include:

- `schemas/council-request.schema.json`
- `schemas/fanout-request.schema.json`
- `schemas/memory-fact-request.schema.json`
- `schemas/tool-result-request.schema.json`

## Reference Fanout Sources

Fanout is adapter-driven. The core models many sources without requiring them at build time:

- `browser_bot`: reference browser lane, compatible with Borai browser-bot `/ask` style adapters.
- `cli_runner_api`: reference CLI/API lane, compatible with CLI-runner `/exec` style adapters and local-provider overlays.
- `living_research`: app-lane package target for research workflows.
- `open_ai_compatible`, `anthropic`, and `custom_http`: neutral extension points.

See `docs/reference-adapters.md` and `examples/living-research/fanout.json`.

## Release

For the public GitHub release process, use `docs/release-guide.md`.
For the validation pass before publishing, use `docs/qa-guide.md`.
For a human-facing usability pass, use `docs/usability-qa-guide.md`.
For `/v1/*` curl snippets, use `docs/api-cheatsheet.md`.
For implementation sweep notes and follow-up watchlist, use `docs/project-sweep.md`.

## Disclaimers

This is an experimental research toolkit. It is not legal, security, financial, or operational advice. Council consensus is an operator decision rule over submitted positions; it is not truth, proof, or blockchain consensus. External agent execution is intentionally adapter-driven and not required by the core crate.

## Make-based Operator Workflow

Most operators can run the same checks in one command with `make`:

```bash
make onboarding   # run doctor + basic CLI smoke + minimal server/API smoke
make doctor      # machine-readable environment/report checks
make shell-completions
make server-up    # starts server on $(SERVER_PORT)
make server-down  # stops server on $(SERVER_PORT)
make smoke        # alias for onboarding (small local evidence pass)
make qa-local     # run scripts/qa-all.sh end-to-end
```

`make onboarding` writes evidence into `${EVIDENCE_DIR}` (default `/tmp/agent-governance-rs-v0.1.0-qa`) and is the quickest reproducible path for first-run verification.

```bash
export EVIDENCE_DIR=/tmp/agent-governance-rs-v0.1.0-qa
make server-up
make server-down
```

## Slash Commands (scripts)

This repository includes a small set of helper scripts under `scripts/` to make common QA and usability workflows repeatable and easy for operators. They are not magical — they're convenience wrappers for the commands shown in the docs. Each script is documented here with usage, environment variables, and troubleshooting notes.

- `scripts/start-server.sh` — start the HTTP server with explicit DB and port
  - Usage: `./scripts/start-server.sh --db /path/to/db.sqlite --port 9797 [--token TOKEN] [--bg]`
  - Behavior: exports `AGENT_GOV_DB` and `AGENT_GOV_TOKEN` then runs `cargo run ... server`.
  - Notes: `--db` is required. Use `--bg` to background the process (shell job control still applies).

- `scripts/start-usability-server.sh` — convenience wrapper for the usability guide
  - Usage: `./scripts/start-usability-server.sh`
  - Behavior: creates `/tmp/agent-governance-rs-v0.1.0-usability` (or `$QA_EVIDENCE`) and starts the server on port `9797` using that directory's `usability.sqlite` DB.
  - Notes: mirror of the `Server First-Run Usability` section in `docs/usability-qa-guide.md`.
- `scripts/onboard.sh` — one-command CLI + API smoke entrypoint.
  - Usage: `./scripts/onboard.sh [EVIDENCE_DIR] [SERVER_PORT] [SERVER_DB]`
  - Behavior: runs `doctor`, core CLI smoke checks, and a minimal server health/auth check.

- `scripts/stop-server-by-port.sh` — stop any local server listening on a given port
  - Usage: `./scripts/stop-server-by-port.sh 9797`
  - Behavior: finds the PID that `ss` reports and sends `TERM` (falls back to `KILL` on failure).
  - Notes: safe for local QA but avoid running on multi-tenant hosts without checking which PID you are killing.

- `scripts/run-usability-checks.sh` — run the small usability smoke tests
  - Usage: `./scripts/run-usability-checks.sh [EVIDENCE_DIR]`
  - Behavior: polls `/health`, performs unauthenticated and authenticated `/v1/council/personas`, and writes evidence into the dir.
  - Notes: server must already be running on `127.0.0.1:9797`.

- `scripts/run-persistence-check.sh` — compare saved before/after persistence JSON
  - Usage: `./scripts/run-persistence-check.sh /tmp/agent-governance-rs-v0.1.0-qa`
  - Behavior: reads `persist-deliberation-before-restart.json` and `persist-deliberation-after-restart.json`, extracts `council_id`, writes `persist-compare.txt` with `PERSIST_OK` or `PERSIST_MISMATCH` and returns non-zero on mismatch.

- `scripts/tar-evidence.sh` — tarball an evidence directory
  - Usage: `./scripts/tar-evidence.sh /tmp/agent-governance-rs-v0.1.0-qa`
  - Behavior: creates `/tmp/agent-governance-rs-v0.1.0-qa.tar.gz` for sharing or archival.

Troubleshooting & tips

- Make sure the script files are executable: `chmod +x scripts/*.sh`.
- Use absolute paths for `--db` to avoid surprises when running from other directories.
- If a server fails to start with `--db` set, confirm that the parent directory exists and is writable (the scripts create evidence dirs for convenience).
- `--bg` backgrounds the cargo process but logs will still be shown in the launching terminal; prefer starting in a dedicated terminal or use a process manager for long-running servers.

If you'd like, I can make a single `scripts/qa-all.sh` that runs the full `docs/qa-guide.md` sequence non-interactively and captures all evidence exactly as the guide prescribes — say “make qa-all”.

I added `scripts/qa-all.sh` to run the main QA steps end-to-end; use it as a convenience but review outputs in the evidence directory before publishing.

## Prompt Examples

Here are concise example prompts you can use when dispatching member prompts, testing the `council run` flow, or exercising adapters. Use them as-is in request files or as starting points to craft member prompts in packs.

- Short analysis (member):

```text
### Restatement
Restate the problem in one or two sentences.

### Analysis
List 3 major considerations and one recommended next action.

### Confidence
Rate confidence: low/medium/high and why.
```

- Cross-examination (member):

```text
Challenge two likely claims from other members, present the strongest opposing view fairly, and give a short update to your position if warranted.
```

- Final position (member):

```text
In 100 words or fewer, state your final position (yes/no/abstain), name any hard dissent, and propose a reversible next action.
```

- Coordinator synthesis (system):

```text
Synthesize final member positions, surface dissenting points, and list unresolved questions with recommended reversible next steps.
```

Tips:Agent orchestration & scheduling: no executor that runs multiple agents in parallel, retries, timeouts, or resource isolation (browser bots, in‑process mock agents, or remote adapters).
Quality evaluation pipeline: no automated post‑hoc evaluation (scoring answers, conflict resolution, cross‑examination automation, provenance scoring) beyond basic consensus thresholding.

- Keep member prompts explicit about format (use headings like `### Restatement`) so downstream parsing and report generation remain deterministic.
- Limit token budgets per member by requesting concise sections and using `max_tokens` at the target level (see `llms[]` in packs).
- For local QA runs, prefer `fast-api` or `browser-reference` targets in examples so the core produces stable, non-networked outputs.

## Executing Prompts — Commands & Example Outputs

Below are concrete commands (CLI and HTTP API) to run a council prompt and inspect the resulting deliberation. Example outputs are condensed to the important fields you will most often check during QA.

1) Run a council prompt locally via the CLI (reads `examples/architecture-council/request.json`):

```bash
cargo run -p agent-governance-cli --bin agent-governance -- council run \
  --file examples/architecture-council/request.json \
  | jq . > /tmp/council-run.json
```

Example (condensed) output written to `/tmp/council-run.json`:

```json
{
  "deliberation": {
    "council_id": "06452d25-a7ce-4fed-9277-b1e7c05cbd31",
    "status": "awaiting_positions",
    "problem": "Should each council member use a different execution lane for this review?",
    "members": [
      { "name": "aristotle", "llm": "fast-api" },
      { "name": "ada", "llm": "browser-reference" }
    ],
    "prompts": [
      { "member": "aristotle", "prompt": "### Restatement\n..." },
      { "member": "ada", "prompt": "### Restatement\n..." }
    ]
  }
}
```

2) Create the same deliberation using the HTTP API (server must be running with `AGENT_GOV_TOKEN`):

```bash
curl -sS http://127.0.0.1:9797/v1/council/deliberations \
  -H 'Authorization: Bearer dev-token' \
  -H 'Content-Type: application/json' \
  --data @examples/architecture-council/request.json \
  | jq . > /tmp/api-deliberation.json
```

Example (condensed) `/tmp/api-deliberation.json`:

```json
{
  "deliberation": {
    "council_id": "06452d25-a7ce-4fed-9277-b1e7c05cbd31",
    "status": "awaiting_positions",
    "prompts": [ { "member": "aristotle", "prompt": "..." } ]
  }
}
```

3) Read back the deliberation and report (after you have a `council_id`):

```bash
COUNCIL_ID=$(jq -r '.deliberation.council_id' /tmp/api-deliberation.json)
curl -sS "http://127.0.0.1:9797/v1/council/deliberations/$COUNCIL_ID" -H 'Authorization: Bearer dev-token' | jq .
curl -sS "http://127.0.0.1:9797/v1/council/deliberations/$COUNCIL_ID/report.md" -H 'Authorization: Bearer dev-token' | sed -n '1,120p'
```

Example readback JSON (condensed):

```json
{
  "deliberation": { "council_id": "06452d25-...", "status": "awaiting_positions", "created_at": "..." }
}
```

4) Submit a context envelope via CLI and verify it via the API (shows redaction & signature fields):

```bash
cargo run -p agent-governance-cli --bin agent-governance -- context sign --file examples/context-envelope/facts.json | tee /tmp/envelope.json
python3 - <<'PY'
import json
e=json.load(open('/tmp/envelope.json'))
print('signature present?', 'signature' in e['envelope'])
PY

curl -sS http://127.0.0.1:9797/v1/context/envelopes/verify \
  -H 'Authorization: Bearer dev-token' -H 'Content-Type: application/json' \
  --data @/tmp/envelope.json | jq .

Note: `jq` expects JSON input. If the server returns non-JSON (for example, a markdown report at a `.md` endpoint or an HTTP error page), piping to `jq` will produce a parse error. Use `--data @file` to send a file's JSON contents, and avoid piping markdown endpoints into `jq`.
```

Example envelope verify output (condensed):

```json
{ "valid": true, "signature_valid": true, "expired": false }
```

Notes:

- Example outputs above are intentionally condensed to key fields (`council_id`, `status`, `prompts`, `signature`). The real CLI/API outputs include more metadata (timestamps, prompt bodies, member targets) that you should archive in your QA evidence directory.
- Use `jq` to extract fields in scripts; the helper scripts in `scripts/` already capture canonical evidence files used by the QA guide.

