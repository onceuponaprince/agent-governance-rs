# GitHub Release Guide

This guide is for publishing `agent-governance-rs` as an experimental public GitHub release on May 22, 2026. It assumes the repo is being split out from the current Borai workspace into a neutral standalone repository.

## Release Goal

Publish `agent-governance-rs` as a Rust-first research toolkit for:

- Dynamic agent council governance loaded from JSON or Markdown frontmatter packs.
- Customizable LLM profiles and per-member model/lane bindings.
- Context envelope signing and verification.
- Memory hygiene, invalidation, and TTL decay.
- Tool result recording, quarantine, and ranking.
- Sandbox-only repair planning.
- Adapter-neutral fanout plans for browser-bot, CLI-runner API lane, living-research, OpenAI-compatible APIs, Anthropic, and custom HTTP adapters.

This release should be positioned as experimental but usable. Avoid claiming that council consensus is truth, formal proof, legal/security advice, or blockchain-style consensus.

## Pre-Release Checklist

Complete these checks before creating the GitHub repository or tag:

- Confirm the package name and repo name are neutral: `agent-governance-rs`.
- Confirm Borai is mentioned only as origin/reference integration, not as the product headline.
- Confirm no generated build directory is present: `agent-governance-rs/target` should not exist.
- Confirm all council personas, domains, prompt templates, and LLM profiles are data-driven through packs, not hard-coded as active runtime policy.
- Confirm examples do not contain real tokens, private URLs, or private workspace names.
- Confirm `Cargo.lock` is committed for reproducible CLI/server builds.
- Confirm `README.md` includes the experimental disclaimer and quickstart commands.
- Confirm `docs/reference-adapters.md` explains browser-bot, CLI-runner API lane, and living-research as reference adapters.

Run:

```bash
cd agent-governance-rs
find . -maxdepth 3 -type d -name target -print
rg -n "sk-[A-Za-z0-9_-]{8,}|ghp_|github_pat_|Authorization: Bearer [^ ]+|api[_-]?key|password|secret-token-value" . \
  --glob '!Cargo.lock'
```

The `rg` command may intentionally match redaction examples such as `secret-token-value`; review any matches before publishing.

## Required Release Checks

Run the full Rust gate from the standalone workspace root:

```bash
cd agent-governance-rs
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Run smoke checks for the main workflows:

```bash
cargo run -q -p agent-governance-cli --bin agent-governance -- personas
cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/request.json
cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/custom-llms.json
cargo run -q -p agent-governance-cli --bin agent-governance -- context sign --file examples/context-envelope/facts.json
cargo run -q -p agent-governance-cli --bin agent-governance -- fanout plan --file examples/living-research/fanout.json
cargo run -q -p agent-governance-cli --bin living-research -- --prompt "Compare release evidence across sources"
```

Expected outcomes:

- `personas` prints members, domains, `llms`, and `default_llm`.
- Council examples return `awaiting_positions` with generated prompts and resolved member targets.
- `custom-llms.json` shows different member LLM lanes and model bindings.
- Context signing redacts obvious secrets and includes a signature.
- Fanout commands produce planned runs only; the core crate should not perform network execution.

## Repository Creation

Create the standalone repository outside the Borai monorepo. Recommended local flow:

```bash
cd /home/onceuponaprince/code/borai/ops/borai-cc
cp -R agent-governance-rs /tmp/agent-governance-rs-release
cd /tmp/agent-governance-rs-release
rm -rf target .git
git init
git add .
git commit -m "Initial experimental release"
```

Create the GitHub repository. If using GitHub CLI:

```bash
gh repo create onceuponaprince/agent-governance-rs \
  --public \
  --description "Experimental Rust toolkit for agent councils, context hygiene, and auditable decision workflows" \
  --source . \
  --remote origin \
  --push
```

If not using GitHub CLI, create the repository in the GitHub UI, then run:

```bash
git remote add origin git@github.com:onceuponaprince/agent-governance-rs.git
git branch -M main
git push -u origin main
```

## GitHub Repository Settings

Set these immediately after pushing:

- About description: `Experimental Rust toolkit for agent councils, context hygiene, and auditable decision workflows.`
- Topics: `rust`, `agents`, `agent-governance`, `multi-agent`, `context-engineering`, `llmops`, `axum`, `sqlite`, `research-toolkit`.
- Website: leave blank unless there is a project page.
- Features: enable Issues and Discussions if you want public feedback; disable Wiki unless you plan to maintain it.
- Default branch: `main`.
- Branch protection: require CI before merge once GitHub Actions is added.
- Security: enable Dependabot alerts and secret scanning if available for the repo.

## Recommended GitHub Actions

Add CI before tagging if time allows. Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Format
        run: cargo fmt --all --check
      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: Test
        run: cargo test --workspace --all-targets
```

If you add this workflow, run it once on `main` before creating the release tag.

## Versioning

The first public release should be `v0.1.0`. Keep all workspace package versions at `0.1.0` for this tag.

Before tagging, verify:

```bash
rg -n "version =|version.workspace" Cargo.toml crates/*/Cargo.toml
cargo metadata --no-deps --format-version 1 | jq '.packages[] | {name, version}'
```

If `jq` is not installed, skip the metadata formatting and inspect `cargo metadata --no-deps --format-version 1` directly.

## Release Commit

Use a concise initial commit message:

```bash
git add .
git commit -m "Release agent-governance-rs v0.1.0"
```

If the initial import is already committed, use:

```bash
git status --short
git add README.md docs examples packs crates Cargo.toml Cargo.lock
git commit -m "Prepare v0.1.0 release"
```

## Tagging

Create an annotated tag:

```bash
git tag -a v0.1.0 -m "agent-governance-rs v0.1.0"
git push origin main
git push origin v0.1.0
```

Do not move a public tag after publishing unless the release is broken and the correction is clearly documented. Prefer `v0.1.1` for fixes.

## GitHub Release Draft

Create a GitHub release from tag `v0.1.0`. Suggested title:

```text
agent-governance-rs v0.1.0
```

Suggested release notes:

```markdown
## Summary

Initial experimental release of `agent-governance-rs`: a Rust toolkit for agent councils, context hygiene, dynamic LLM/profile governance, fanout planning, and auditable decision workflows.

## Highlights

- Dynamic council packs loaded from JSON or Markdown frontmatter.
- Customizable LLM profiles with per-member bindings and request-time overrides.
- Council prompt generation and submitted-position consensus reporting.
- Signed context envelopes with verification and tamper rejection.
- Memory fact TTL, scoring, and source invalidation.
- Tool result recording, quarantine, ranking, and probe recovery.
- Sandbox-only repair plan generation.
- Axum `/v1/*` server with bearer auth by default and optional SQLite persistence.
- CLI package with `agent-governance` and `living-research` binaries.
- Reference adapter docs for browser-bot, CLI-runner API lane, living-research, OpenAI-compatible APIs, Anthropic, and custom HTTP.

## Install From Source

```bash
git clone https://github.com/onceuponaprince/agent-governance-rs.git
cd agent-governance-rs
cargo test --workspace --all-targets
cargo run -p agent-governance-cli --bin agent-governance -- personas
```

## Disclaimer

This is an experimental research toolkit. It is not legal, security, financial, or operational advice. Council consensus is an operator decision rule over submitted positions; it is not truth, formal proof, or blockchain consensus. External agent execution is adapter-driven and not required by the core crate.
```

Attach source archives generated by GitHub automatically. Do not attach compiled binaries unless you also define a repeatable build process for each target.

## Optional Binary Artifacts

For a source-only v0.1.0, skip binary artifacts. If you want binaries today, build only after CI is green and clearly label the target triples. Example local builds:

```bash
cargo build --release --workspace
mkdir -p dist
cp target/release/agent-governance dist/agent-governance-linux-x86_64
cp target/release/living-research dist/living-research-linux-x86_64
cp target/release/agent-governance-server dist/agent-governance-server-linux-x86_64
sha256sum dist/* > dist/SHA256SUMS
```

Only upload binaries that were built on a clean machine or clean container.

## crates.io Publishing

Publishing to crates.io is optional for v0.1.0. GitHub-only is acceptable for an experimental release.

If publishing crates today, publish in dependency order:

```bash
cargo publish -p agent-governance-core --dry-run
cargo publish -p agent-governance-core

cargo publish -p agent-governance-server --dry-run
cargo publish -p agent-governance-server

cargo publish -p agent-governance-cli --dry-run
cargo publish -p agent-governance-cli
```

Before publishing, confirm crate metadata is final:

- `description` is accurate.
- `license` is set.
- `repository` points to the public GitHub repo.
- README examples work from a clean clone.
- Pack data required by `include_str!` is included in the crate package.

Check package contents:

```bash
cargo package -p agent-governance-core --list
cargo package -p agent-governance-server --list
cargo package -p agent-governance-cli --list
```

If the pack files are missing from `agent-governance-core` package contents, add explicit `include` metadata before publishing.

## Post-Release Validation

After the GitHub release is public, test from a clean clone:

```bash
cd /tmp
rm -rf agent-governance-rs-smoke
git clone https://github.com/onceuponaprince/agent-governance-rs.git agent-governance-rs-smoke
cd agent-governance-rs-smoke
git checkout v0.1.0
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -q -p agent-governance-cli --bin agent-governance -- council run --file examples/architecture-council/custom-llms.json
```

Start the server locally:

```bash
AGENT_GOV_TOKEN=dev-token cargo run -p agent-governance-cli --bin agent-governance -- server --db ./agent-governance.sqlite
```

In another terminal:

```bash
curl -sS http://127.0.0.1:9797/health
curl -sS http://127.0.0.1:9797/v1/council/personas \
  -H 'Authorization: Bearer dev-token' | head
```

Confirm unauthenticated `/v1/*` requests are rejected unless `--dev-no-auth` is explicitly used.

## Announcement Copy

Short announcement:

```text
Released agent-governance-rs v0.1.0: an experimental Rust toolkit for dynamic agent councils, custom LLM profiles, context hygiene, tool quarantine, repair planning, and adapter-neutral fanout workflows.

GitHub: https://github.com/onceuponaprince/agent-governance-rs
```

Longer announcement:

```text
I released agent-governance-rs v0.1.0 today. It is an experimental Rust toolkit for agent governance: dynamic council packs, customizable LLM profiles per member, context envelope signing, memory invalidation, tool quarantine/ranking, sandbox-only repair plans, and fanout planning across browser, CLI, API, and app sources.

The core crate does not execute external agents. It keeps execution adapter-driven, with reference docs for browser-bot, CLI-runner API lane, living-research, OpenAI-compatible APIs, Anthropic, and custom HTTP adapters.

GitHub: https://github.com/onceuponaprince/agent-governance-rs
```

## Rollback Plan

If the release is wrong but no one has consumed it yet:

- Mark the GitHub release as draft or delete the release page.
- Leave the tag alone if it may have been fetched.
- Publish a corrective `v0.1.1` tag instead of rewriting history.

If secrets were accidentally published:

- Immediately revoke the secret at the provider.
- Remove the secret from Git history using a standard history rewrite tool.
- Force-push only after understanding downstream impact.
- Publish a clear incident note if the repo was already public.

## Final Go/No-Go

Go if all are true:

- Formatting, clippy, and tests pass.
- Clean clone smoke works.
- README accurately describes experimental status.
- Dynamic council packs and LLM profiles are documented.
- No generated `target/` directory or secrets are included.
- GitHub release notes include disclaimers and source install steps.

No-go if any are true:

- Any hard-coded private provider, model, workspace, token, or endpoint is required for the core examples.
- Auth can be bypassed on `/v1/*` without `--dev-no-auth`.
- Pack files are missing from the published crate package.
- The release notes imply consensus equals truth, formal proof, or production safety.
