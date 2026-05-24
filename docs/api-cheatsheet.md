# HTTP API cheatsheet

Assumptions:

- Server on `http://127.0.0.1:9797` (default `agent-governance server` bind address).
- Bearer auth: set `AGENT_GOV_DEV=1`, `AGENT_GOV_TOKEN=dev-token`, pass `--dev`, and send `Authorization: Bearer dev-token` on `/v1/*` (unless the server was started with `--dev-no-auth` for local demos only).

```bash
export BASE=http://127.0.0.1:9797
AUTH=( -H 'Authorization: Bearer dev-token' )
```

## Health (no auth)

```bash
curl -sS "$BASE/health"
```

## Council

```bash
curl -sS "${AUTH[@]}" "$BASE/v1/council/personas"

curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' \
  --data @examples/architecture-council/request.json \
  "$BASE/v1/council/deliberations"
```

Read-back (replace `COUNCIL_ID` from the create response `deliberation.council_id`):

```bash
curl -sS "${AUTH[@]}" "$BASE/v1/council/deliberations/COUNCIL_ID"
curl -sS "${AUTH[@]}" "$BASE/v1/council/deliberations/COUNCIL_ID/report.md"
```

## Context envelopes

Create:

```bash
curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' \
  --data @examples/context-envelope/facts.json \
  "$BASE/v1/context/envelopes" | tee /tmp/envelope-create.json
```

Verify (pipe the `envelope` object from the create response):

```bash
python3 - <<'PY' | curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' \
  --data @/dev/stdin "$BASE/v1/context/envelopes/verify"
import json
from pathlib import Path
body = json.loads(Path("/tmp/envelope-create.json").read_text())
print(json.dumps({"envelope": body["envelope"]}))
PY
```

## Memory

POST a fact:

```bash
curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' \
  --data @examples/memory-invalidation/fact.json \
  "$BASE/v1/memory/facts"
```

List active facts:

```bash
curl -sS "${AUTH[@]}" "$BASE/v1/memory/facts"
```

Invalidate:

```bash
curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' \
  --data @examples/memory-invalidation/invalidate.json \
  "$BASE/v1/memory/invalidations"
```

## Tools

```bash
curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' \
  --data @examples/tool-quarantine/result-probe.json \
  "$BASE/v1/tools/results"

curl -sS "${AUTH[@]}" "$BASE/v1/tools/rank"
curl -sS "${AUTH[@]}" "$BASE/v1/tools/browser_ask"
```

## Repair

```bash
curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' \
  --data '{"failure":"selector timeout in provider UI","target":"browser-bot"}' \
  "$BASE/v1/repair/plans"
```

## Fanout

```bash
curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' \
  --data @examples/living-research/fanout.json \
  "$BASE/v1/fanout/plans" | tee /tmp/fanout-create.json
```

Read-back (`plan.run_id` from the POST response):

```bash
PLAN_ID="$(python3 -c "import json;print(json.load(open('/tmp/fanout-create.json'))['plan']['run_id'])")"
curl -sS "${AUTH[@]}" "$BASE/v1/fanout/plans/$PLAN_ID"
curl -sS "${AUTH[@]}" "$BASE/v1/fanout/plans/$PLAN_ID/report.md"
```

## See also

- `README.md` — full route list and quickstart.
- `agent-governance doctor` — lists bundled `examples/` and `packs/` files when run from the repository root.
