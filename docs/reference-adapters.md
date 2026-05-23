# Reference Adapters

`agent-governance-rs` keeps fanout adapter-neutral. The core crate records source declarations and planned dispatch metadata; production execution belongs in adapters owned by the host application.

## browser-bot

Use `adapter: "browser_bot"` for browser-backed providers. A host adapter can map a source to Borai browser-bot's `/ask` contract with provider, workspace, prompt, optional thread id, and auth headers. Browser login, selector health, and stale-profile handling stay outside the core crate.

## CLI-runner API Lane

Use `adapter: "cli_runner_api"` for CLI or local-model execution through a service shaped like CLI-runner `/exec`. The source can include a CLI/provider name, endpoint, model, and local-provider metadata. CLI policy, binary availability, fallback chains, and sandbox details stay in the adapter service.

## living-research

The CLI package includes a `living-research` binary that emits an app-lane fanout plan. Treat it as a reference app package target: it can fan out to browser-bot, CLI-runner, API sources, and custom research stores while preserving the same governance records.

## Many Sources

A fanout plan can mix browser, CLI, API, and app sources. The plan captures source identity, adapter type, endpoint, provider, workspace, model, prompt hash, and recommended next actions. The core crate does not call the network.

## LLM Profiles

Council packs and council requests can declare `llms[]` entries. Each profile names a lane, provider, model, endpoint, optional workspace, sampling parameters, token limit, and arbitrary metadata. Members bind to profiles with `llm`; `default_llm` fills in members that do not specify one. This keeps model selection outside Rust code while preserving a resolved target in each council run.
