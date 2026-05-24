# Project Sweep Notes

This document records the release-readiness sweep for usability, abstraction, optimization, and dangling implementation risk.

## Scope

Reviewed:

- CLI command surfaces and examples.
- Axum server routes, auth boundary, SQLite startup behavior, and persistence readback.
- Core council, context envelope, fanout, memory, repair, and tool quarantine modules.
- README, release guide, technical QA guide, and reference adapter docs.
- Package contents and default pack embedding.

## Fixes Applied

- Added `agent-governance doctor` (lists example/pack paths and copy-paste starters from the CLI).
- Added `docs/api-cheatsheet.md` with curl-focused `/v1/*` examples aligned to repository `examples/`.
- Added a human-facing usability QA guide at `docs/usability-qa-guide.md`.
- Standardized local API QA examples on `127.0.0.1:9797`.
- Added `AGENT_GOV_DB` support to the `agent-governance server` subcommand.
- Shared SQLite URL construction through `agent-governance-server` instead of duplicating format strings.
- Ensured fresh SQLite DB files are created with `create_if_missing(true)`.
- Added a server test for fresh SQLite DB creation.
- Removed an allocation from tool-state calculation.
- Compute fanout prompt hashes once per plan instead of once per source.
- Deduplicate council positions by member, using the latest submitted position for consensus.
- Simplified request target resolution and normalized requested domain names.
- Replaced SQLite schema match/unreachable logic with static schema statements.
- Added `src/default_council.json` to the core crate so packaged builds include the embedded default council pack.
- Expanded README API route list to include tool state and fanout read/report routes.
- Added `.gitignore` coverage for local DBs, build output, env files, logs, and key material.

## Implementation Notes

- Core fanout remains intentionally non-executing. It plans adapter dispatch and records metadata; real network execution belongs in adapter integrations.
- Council output intentionally returns `awaiting_positions` when no positions are submitted. This avoids implying an LLM answered locally.
- The SQLite persistence model currently supports durable readback for deliberations and fanout plans by ID. Memory and tool stores are persisted as event rows but in-memory ranking/listing state is not hydrated on restart in `v0.1.0`.
- Context envelope verification signs the canonical envelope minus `signature`; tampering facts after signing correctly returns `signature_valid: false`.
- Repair plans are sandbox-only unless `allow_production_patch` is explicitly set.

## Watchlist For Later Releases

- Completed: dedicated `agent-governance council submit-positions` command for second-pass position submission.
- Completed: API routes for listing recent deliberations and fanout plans when SQLite is enabled.
- Completed: optional hydration of memory/tool operational state from SQLite on startup (`AGENT_GOV_HYDRATE_STATE=true`).
- Completed: JSON schema generation command and generated schema files under `schemas/`.
- Completed: clearer custom council pack examples in the README.
- Completed: CI workflow added before tagging public releases.

## No Blocking Dangling Implementations Found

No `TODO`, `FIXME`, `todo!`, `unimplemented!`, `unreachable!`, or direct `panic!` markers were found in release code. Remaining `unwrap`/`expect` usage is limited to tests, static regex/default-pack initialization, and HMAC construction where the library accepts any key length.
