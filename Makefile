SHELL := /bin/bash

EVIDENCE_DIR ?= /tmp/agent-governance-rs-v0.1.0-qa
SERVER_PORT ?= 9797
SERVER_DB ?= $(EVIDENCE_DIR)/agent-governance.sqlite
SERVER_TOKEN ?= dev-token

.PHONY: help doctor quickstart completion shell-completions server-up server-down onboarding smoke qa-local clean clean-evidence

help:
	@echo "agent-governance-rs convenience targets"
	@echo "  make doctor         run environment checks (cargo/rustc/required files)"
	@echo "  make quickstart     print the human quickstart flow"
	@echo "  make shell-completions [OUT=path]  generate bash completion at $$OUT"
	@echo "  make onboarding      run one-command onboarding/first-pass QA checks"
	@echo "  make server-up       start local server in background"
	@echo "  make server-down     stop local server by port"
	@echo "  make smoke           run CLI smoke checks into $(EVIDENCE_DIR)"
	@echo "  make qa-local        run full qa-all flow"
	@echo "  make clean-evidence  remove evidence directory"

doctor:
	@cargo run -q -p agent-governance-cli --bin agent-governance -- doctor --json > $(EVIDENCE_DIR)/doctor.json
	@echo "doctor report written: $(EVIDENCE_DIR)/doctor.json"

quickstart:
	@cargo run -q -p agent-governance-cli --bin agent-governance -- quickstart

shell-completions:
	@mkdir -p completions
	@cargo run -q -p agent-governance-cli --bin agent-governance -- completion bash > completions/agent-governance.bash
	@cargo run -q -p agent-governance-cli --bin agent-governance -- completion zsh > completions/agent-governance.zsh
	@echo "shell completion files written to completions/"

server-up:
	@mkdir -p "$(EVIDENCE_DIR)"
	@./scripts/start-server.sh --port $(SERVER_PORT) --db $(SERVER_DB) --token $(SERVER_TOKEN) --bg

server-down:
	@./scripts/stop-server-by-port.sh $(SERVER_PORT)

onboarding: 
	@./scripts/onboard.sh

smoke:
	@$(MAKE) -s onboarding

qa-local:
	@./scripts/qa-all.sh $(EVIDENCE_DIR)

clean-evidence:
	@rm -rf $(EVIDENCE_DIR)

clean:
	@rm -rf target
	rm -f test_traces.sqlite
	rm -rf .coverage
	@echo "project clean done"
