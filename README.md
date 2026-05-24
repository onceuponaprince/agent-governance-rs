# agent-governance-rs

Experimental Rust toolkit for agent councils, context hygiene, and auditable decision workflows.

Originally extracted from Borai's agent infrastructure, this repo keeps Borai, browser-bot, CLI-runner, and living-research as reference integrations rather than product assumptions. The core crate does not execute external agents; it builds governance records, prompt rounds, context envelopes, memory hygiene state, tool quarantine rankings, repair plans, and adapter-neutral fanout plans.

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

Server APIs include:

- `GET /v1/council/personas`
- `POST /v1/council/deliberations`
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
- `GET /v1/fanout/plans/{id}`
- `GET /v1/fanout/plans/{id}/report.md`

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
