# Human-Facing Usability QA Guide

Use this guide after the technical QA pass to check whether `agent-governance-rs` is understandable and usable by a human operator. This is not a release gate for Rust correctness; it is a release gate for clarity, first-run confidence, and workflow fit.

## Audience

Run this pass as someone who did not build the project but understands basic Rust CLI workflows. The operator should be able to answer: what is this, what does it do locally, what does it not do, and what evidence should I trust?

## Setup

Start from a clean shell:

```bash
cd /path/to/agent-governance-rs
export QA_EVIDENCE=/tmp/agent-governance-rs-usability
mkdir -p "$QA_EVIDENCE"
```

Expected result: every later command can be copied without inventing paths or environment variables.

## First Impression

Open `README.md` and review only the first screen plus the package list.

Pass criteria:

- The first paragraph explains the project in one or two reads.
- It is clear that the core crate does not execute external agents.
- Package names map to obvious roles: core logic, server API, CLI/demo binaries.
- The reader can tell this is experimental and source-first.

Fail criteria:

- The reader expects browser-bot, CLI-runner, OpenRouter, or Anthropic calls to happen automatically.
- The reader cannot tell which command to run first.
- The disclaimer is missing or sounds like production assurance.

## First Command

Run:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- personas \
  | tee "$QA_EVIDENCE/personas.json"
```

Pass criteria:

- The command finishes without credentials.
- Output is valid JSON.
- The catalog includes member names, domains, `llms`, and `default_llm`.
- The operator can explain that these are data-driven council pack entries.

## Council Workflow Comprehension

Run:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- council run \
  --file examples/architecture-council/request.json \
  | tee "$QA_EVIDENCE/council.json"
```

Pass criteria:

- The output status is `awaiting_positions`.
- The operator understands that generated prompts are meant to be dispatched elsewhere.
- The output does not imply that a real LLM answered.
- The recommended next action is clear: collect independent positions and resubmit with `positions[]`.

Human comprehension check:

Ask the operator to answer in plain language: "What should I do after this council run?" A passing answer mentions dispatching prompts, collecting positions, and using the `council_id` as evidence.

## Custom LLM Profile Usability

Run:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- council run \
  --file examples/architecture-council/custom-llms.json \
  | tee "$QA_EVIDENCE/custom-llms.json"
```

Pass criteria:

- `aristotle` resolves to `fast-api`.
- `ada` resolves to `browser-reference`.
- The operator can see that the repo models targets without requiring credentials.

Fail criteria:

- It looks like provider credentials are required for this local check.
- The target resolution is hidden or ambiguous.

## Context Envelope Usability

Run:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- context sign \
  --file examples/context-envelope/facts.json \
  | tee "$QA_EVIDENCE/context-envelope.json"
```

Pass criteria:

- Output includes `signature`, `fact_hashes`, `expires_at`, and `trust_role`.
- The literal `secret-token-value` does not appear in output.
- The operator understands that redacted facts remain auditable through hashes.

## Fanout Expectation Check

Run:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- fanout plan \
  --file examples/living-research/fanout.json \
  | tee "$QA_EVIDENCE/fanout.json"
```

Pass criteria:

- Every result has `status: "planned"`.
- `ok: false` is understood as "not executed yet," not a failure.
- Result metadata says `execution: "adapter-driven outside core crate"`.

Fail criteria:

- The operator treats planned fanout output as a failed network run.
- The operator expects source citations from this deterministic planning command.

## Server First-Run Usability

Note: the server will create a SQLite database file at `$QA_EVIDENCE/usability.sqlite` when started with the `--db` flag below. This file is for local usability evidence only; you can remove it after the check with `rm -f "$QA_EVIDENCE/usability.sqlite"`.

Start the server in one terminal:

```bash
export AGENT_GOV_DB="$QA_EVIDENCE/usability.sqlite"
rm -f "$AGENT_GOV_DB"
AGENT_GOV_DEV=1 AGENT_GOV_TOKEN=dev-token cargo run -q -p agent-governance-cli --bin agent-governance -- server \
  --bind 127.0.0.1:9797 \
  --dev \
  --db "$AGENT_GOV_DB"
```

In another terminal:

```bash
# Same QA_EVIDENCE as in Setup, or export it again in this terminal.
curl -sS http://127.0.0.1:9797/health | tee "$QA_EVIDENCE/health.json"
curl -sS -i http://127.0.0.1:9797/v1/council/personas | tee "$QA_EVIDENCE/auth-rejection.txt"
curl -sS http://127.0.0.1:9797/v1/council/personas \
  -H 'Authorization: Bearer dev-token' \
  | tee "$QA_EVIDENCE/api-personas.json"
```

Pass criteria:

- SQLite file is created automatically.
- `/health` works without auth and includes operational fields (`version`, `api_auth`).
- `/v1/*` rejects missing auth with `authentication_required`.
- The authenticated request returns JSON without extra setup.

Stop the server with `Ctrl-C` after this check.

## Documentation Navigation

Review the repository as if browsing GitHub.

Pass criteria:

- README links to release, technical QA, usability QA, HTTP curl cheatsheet, and reference adapter docs.
- `docs/release-guide.md` explains publication.
- `docs/qa-guide.md` explains operator release validation.
- `docs/usability-qa-guide.md` explains human usability validation.
- Examples are named by workflow, not by internal implementation detail.

## Error Message Check

Intentionally run a bad command:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- council run \
  --file examples/architecture-council/missing.json
```

Pass criteria:

- The error says which file could not be read.
- The operator can correct the path without reading source code.

## Go/No-Go

Go if all are true:

- A new operator can run the first CLI command in under five minutes.
- The operator can explain planned fanout versus executed fanout.
- The operator can start the server and understand auth behavior.
- The operator does not confuse redaction examples with real secrets.
- The operator can find release and QA docs from README.

No-go if any are true:

- The local examples appear to require live provider credentials.
- `ok: false` planned fanout output is likely to be read as an error.
- Server DB setup requires unexplained shell state.
- README and QA docs disagree on default ports or command names.
- The project claims more production readiness than the code provides.
