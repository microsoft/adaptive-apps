# Generate Radius artifacts with the `ada` CLI

`ada package` packages an application's source folder into a Radius
application definition (`app.bicep`). It supports several **generation
strategies** — from a fully deterministic analyzer to an LLM driven by the
upstream [Radius app-modeling skill](https://github.com/radius-project/radius-skills)
— and an optional **critic loop** that iterates on the result until it
passes review.

This tutorial uses the bundled [`src/`](../../src) trading app as the
worked example and compares the strategies side by side.

## 0. Prerequisites

* `ada` installed — see [`common/prepare-cli.md`](../common/prepare-cli.md).
* (LLM / skill strategies only) An OpenAI-compatible API key in
  `OPENAI_API_KEY`, or pass `--llm-api-key`.
* (skill strategy only) Network access to GitHub to fetch the skill, plus
  optionally `GITHUB_TOKEN` to raise the API rate limit.
* (critic loop only, recommended) `rad`, `bicep`, or `az` on `PATH` so the
  deterministic compile critic can run.

Clone the repo (or use your install) and work from the repo root so the
example paths line up:

```bash
git clone https://github.com/microsoft/adaptive-apps.git
cd adaptive-apps
```

## 1. The example app

[`src/`](../../src) is a multi-component "trading" app defined in a single
[`docker-compose.yml`](../../src/docker-compose.yml):

| Compose service | Role | How `ada` maps it |
| --- | --- | --- |
| `postgres` | data store | → `Radius.Resources/postgreSqlDatabases` |
| `mosquitto` | MQTT broker | → `Radius.Resources/mqttBrokers` |
| `backend` | C# API | → `Applications.Core/containers` (connects to db + mqtt) |
| `ai-agent` | C# AI assistant | → `Applications.Core/containers` |
| `frontend` | Node.js UI | → `Applications.Core/containers` (connects to mqtt) |
| `otel-collector`, `prometheus`, `zipkin` | observability | **skipped** — provided by the platform portfolio |

The mapping rules (postgres/MQTT → custom resources, observability infra →
skipped, everything else → containers) are what the deterministic strategy
applies; the LLM and skill strategies refine on top of the same plan.

> **Is a `docker-compose.yml` required?** Only for the `compose` strategy
> — it has nothing else to analyze. The `llm` and `skill` strategies treat
> compose as one optional signal: when it's missing they model directly
> from the source bundle (package manifests like `package.json`,
> `*.csproj`, `pyproject.toml`, `go.mod`; Dockerfiles; and READMEs). If a
> compose file *is* present, all strategies use it.

## 2. Resource-type grounding (shared by every strategy)

Before generating, `ada package` loads this repo's **resource-type
catalog** — the types defined in
[`radius/resource-types/types.yaml`](../../radius/resource-types/types.yaml)
(`Radius.Resources/postgreSqlDatabases`, `mqttBrokers`, `aiModels`,
`workloadIdentities`, `idProviders`, `governance`, `agentGuardrails`). The
catalog is resolved in this order:

1. an explicit `--types <path>`;
2. the repo-local `radius/resource-types/types.yaml` (found by walking up
   from the input folder / working directory);
3. the installed copy at `$ADA_HOME/radius/resource-types/types.yaml`.

The catalog grounds **all three** strategies:

* **compose** emits the exact `type@apiVersion` from the catalog;
* **llm / skill** receive a summary of the available types and are told to
  use *only* those (the upstream skill otherwise points at a different
  registry that does not match this platform);
* the **critic loop** adds a deterministic check that rejects any
  `Radius.Resources/*` type or api-version not in the catalog.

When `ada` finds the catalog it prints a line like:

```text
  types       : Radius.Resources (7 types from .../radius/resource-types/types.yaml)
```

If you see `note: no resource-type catalog found …`, pass `--types` or run
`ada init` to stage the artifacts under `$ADA_HOME`.

## 3. Strategy A — `compose` (default, deterministic)

No API key, no network — a pure function of the compose file. Best for a
fast, reproducible starting point and for CI.

> This is the **one** strategy that requires a `docker-compose.yml`. If
> none is found it stops with a hint to switch strategies:
>
> ```text
> Error: no docker-compose file found under src. The 'compose' strategy
> requires one; try '--strategy llm' or '--strategy skill' to model from
> source instead.
> ```

```bash
ada package --input src --output app.bicep --strategy compose
```

> `--strategy compose` is the default, so `ada package -i src -o app.bicep`
> is equivalent. Use `--dry-run` to print to stdout instead of writing the
> file.

The run summary shows the analysis plan, then writes `app.bicep`:

```text
-> analyzing docker-compose services
  application: src
  resources :
    - src-mqtt (mqtt)
    - src-postgres (postgres)
  containers:
    - ai-agent :7000
    - backend :8080 -> srcMqtt,srcPostgres
    - frontend :3000 -> srcMqtt
  skipped (platform infra): otel-collector, prometheus, zipkin
```

Excerpt of the generated `app.bicep` — note the catalog-qualified type
references and the wired `connections`:

```bicep
resource srcPostgres 'Radius.Resources/postgreSqlDatabases@2025-08-01-preview' = {
  name: 'src-postgres'
  properties: {
    environment: environment
    application: srcApp.id
    size: 'S'
  }
}

resource backend 'Applications.Core/containers@2023-10-01-preview' = {
  name: 'backend'
  properties: {
    application: srcApp.id
    container: {
      image: '${imageRegistry}/backend:${imageTag}'
      ports: {
        http: { containerPort: 8080 }
      }
      env: {
        CONNECTION_DB_HOST: { value: 'postgres' }
        // …
      }
    }
    connections: {
      mqtt: { source: srcMqtt.id }
      db: { source: srcPostgres.id }
    }
  }
}
```

**Strengths:** fast, free, reproducible, offline.
**Limits:** rule-based — it carries compose env vars through verbatim
(including ones a connection would auto-inject) and won't infer
app-specific resources beyond postgres/MQTT.

## 4. Strategy B — `llm` (single-shot, self-authored prompt)

Sends the analyzer plan (if a compose file is present), a starter bicep, a
bundle of source signals (package manifests, Dockerfiles, READMEs), and the
resource-type catalog to an OpenAI-compatible endpoint, asking it to refine
the result.

```bash
export OPENAI_API_KEY=sk-…

ada package \
  --input src \
  --output app.llm.bicep \
  --strategy llm
```

Configure the endpoint/model as needed (defaults shown):

```bash
ada package -i src -o app.llm.bicep --strategy llm \
  --llm-endpoint https://api.openai.com/v1/chat/completions \
  --llm-model gpt-4o-mini
```

> `--llm` is kept as a hidden backward-compatible alias for
> `--strategy llm`.
>
> **No compose file?** This strategy still works — it models from the
> source bundle and prints `compose : (none — modeling from source)`.

**Strengths:** cleans up env vars, infers ports/connections from source,
adapts to apps the static rules don't cover.
**Limits:** uses a generic built-in prompt — it knows the platform's types
(via the catalog) but not the platform's *conventions* (naming, secret
handling, structure). That's what the skill strategy adds.

## 5. Strategy C — `skill` (driven by the Radius app-modeling skill)

The [`radius-project/radius-skills`](https://github.com/radius-project/radius-skills)
repo publishes an `app-modeling` skill: a set of authoritative rules
(naming conventions, structure, validation checklist) that an agent
follows to produce consistent, high-quality `app.bicep`. The `skill`
strategy uses that skill as its system prompt.

### 5.1 Fetch the skill

Skills live in a separate repo and evolve independently, so `ada` fetches
and caches them with a pinned commit SHA (reproducible + offline-friendly):

```bash
ada skill add radius-project/radius-skills
```

```text
-> resolving ref
-> listing skills/ tree @ 88fb944
-> downloading 7 files
✓ cached 7 files at radius-project/radius-skills (sha 88fb944)
```

Inspect what's cached and the pinned SHA:

```bash
ada skill list
```

```text
cached skills
  cache       : ~/.adaptive/skills
  radius-project/radius-skills  ref=main  sha=88fb944  files=7
      - app-modeling
```

The files land under `$ADA_HOME/skills/<owner>/<repo>/<skill>/` and the
resolved SHA is recorded in `skills.lock`. Generation always pins to that
SHA, so the same input produces the same prompt.

### 5.2 Keep the skill up to date

`ada skill update` re-resolves the recorded ref and refreshes the cache
only when upstream moved:

```bash
ada skill update                       # all cached repos
ada skill update radius-project/radius-skills   # just one
```

```text
-> resolving ref
✓ up to date (88fb944)
```

> For automation, run `ada skill update` on a schedule (e.g. a GitHub
> Action) and open a review-gated PR when the locked SHA changes — never
> auto-bump into generation.

### 5.3 Generate

```bash
ada package \
  --input src \
  --output app.skill.bicep \
  --strategy skill
```

`ada` loads the cached pack, echoes the pinned SHA, and drives the model
with the skill's rules (its chat/PR choreography is suppressed so you get
bicep only):

```text
-> loading skill radius-project/radius-skills/app-modeling
  skill sha   : 88fb944
```

Point at a different skill repo/name if needed:

```bash
ada package -i src -o app.skill.bicep --strategy skill \
  --skill radius-project/radius-skills \
  --skill-name app-modeling
```

> **No compose file?** Like `llm`, the `skill` strategy models from the
> source bundle when compose is absent.

**Strengths:** highest fidelity to Radius best practices and naming.
**Limits:** depends on an external skill (fetch + cache it first) and an
API key.

## 6. The critic loop (`--critic`) — iterate to higher quality

The "cross-critic" mode runs a **generate → critique → refine** loop. Each
candidate is checked by a set of critics; their issues are fed back to the
generator until everything passes or `--max-iters` is reached (the best
candidate is always emitted, never lost).

```bash
ada package \
  --input src \
  --output app.bicep \
  --strategy skill \
  --critic \
  --max-iters 3
```

The critic set (assembled automatically):

| Critic | Type | What it checks |
| --- | --- | --- |
| resource-type catalog | deterministic | every `Radius.Resources/*` type + api-version exists in the catalog |
| `rad`/`bicep`/`az` build | deterministic | the candidate actually compiles |
| LLM checklist judge | LLM | scores against the skill's own validation checklist (or a built-in rubric) |
| truth-ref regression | LLM | only added when `--truth-ref` is set: flags resources/connections/params dropped or changed vs the baseline without a source-driven reason |

A loop iteration looks like:

```text
-> iteration 1/3: generating candidate
✓ critic [resource-type catalog] passed
warn: critic [rad bicep build] found 2 issue(s)
  - app.bicep(42,5) : Error BCP …
✓ critic [llm checklist judge] passed
-> iteration 2/3: generating candidate
✓ critic [resource-type catalog] passed
✓ critic [rad bicep build] passed
✓ critic [llm checklist judge] passed
✓ all critics passed on iteration 2
```

> `--critic` only applies to the `llm` and `skill` strategies (the
> `compose` output is deterministic, so there's nothing to iterate). If no
> `rad`/`bicep`/`az` is found on `PATH`, the compile critic is skipped with
> a note and the loop continues with the remaining critics.

### Updating an existing app (`--truth-ref` + `--explain`)

When you already have a known-good `app.bicep` and want to regenerate it
against changed source (a new service, updated image, removed dependency),
pass the existing file as the **truth reference**. It is used three ways:

1. **Grounding** — the baseline is added to the generation prompt as the
   authoritative source for structure, naming, api-versions, and wiring;
   the model preserves what still applies and changes only what the source
   requires.
2. **Regression critic** — under `--critic`, a `truth-ref regression`
   critic flags anything dropped or altered without a source-driven reason
   (additions and intentional updates are allowed).
3. **Explanation** — `--explain` prints an `Added / Removed / Changed /
   Risk` summary of how the result diverges from the baseline and why.

```bash
ada package \
  --input src \
  --output app.bicep \
  --strategy skill \
  --truth-ref radius/app.bicep \
  --critic \
  --explain
```

With `--explain`, the run ends with a human-readable diff so you can see
exactly what changed before committing the regenerated file:

```text
Divergence from --truth-ref

## Added
- container `notifier` — new service detected in src/notifier (Dockerfile + package.json).
- connection `mqtt` on `backend` — wired to the existing mqttBroker for the new event flow.

## Removed
- container `legacy-worker` — no longer present in docker-compose / source bundle.

## Changed
- `frontend` image tag inferred from build context; containerPort 8080 → 3000 (matches server.js).

## Risk / review notes
- `notifier` has no resource limits; confirm before deploying to a shared cluster.
```

> `--truth-ref` and `--explain` need an API key (they drive the LLM). The
> `compose` strategy ignores `--truth-ref` for *generation* (its output is
> deterministic) but still uses it for `--explain`. `--explain` requires
> `--truth-ref`.

## 7. Compare the strategies

| | `compose` | `llm` | `skill` |
| --- | --- | --- | --- |
| Requires a compose file | **yes** | no (optional signal) | no (optional signal) |
| API key required | no | yes | yes |
| Network required | no | yes (LLM) | yes (LLM + skill fetch) |
| Reproducible | yes | approximately (temp 0.1) | approximately, SHA-pinned skill |
| Knows platform **types** | yes (catalog) | yes (catalog) | yes (catalog) |
| Knows platform **conventions** | rules only | generic prompt | full skill ruleset |
| Refines env/ports from source | no | yes | yes |
| Works with `--critic` | n/a | yes | yes |
| Best for | CI, fast starting point | quick refinement | highest-quality output |

A practical workflow: start with `compose` for a quick, free baseline;
switch to `skill --critic` when you want a polished, best-practice
`app.bicep` to commit.

## 8. Deploy the generated artifact

The generated `app.bicep` is a normal Radius app definition. Register the
resource types and deploy it into a Radius environment (see
[`getting-started`](../getting-started/README.md) for full platform setup):

```bash
rad resource-type create -f radius/resource-types/types.yaml
rad deploy app.bicep \
  --group adaptive \
  --environment trading \
  --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
  --parameters imageTag=latest \
  --parameters otelCollectorEndpoint=http://otel-collector.$NAMESPACE:4318
```

> The generated file is a **starting point** — review it, then add the
> OIDC / workload-identity / AI parameters your portfolio needs. The
> hand-authored reference app at [`radius/app.bicep`](../../radius/app.bicep)
> shows the fully wired version.

## Command reference

```bash
# Generation
ada package -i <src> -o <app.bicep> [--strategy compose|llm|skill]
                                    [--types <types.yaml>]
                                    [--critic [--max-iters N]]
                                    [--truth-ref <app.bicep>] [--explain]
                                    [--dry-run]
                                    [--skill <owner/repo>] [--skill-name <name>]
                                    [--llm-endpoint URL] [--llm-model M] [--llm-api-key KEY]

# Skill management
ada skill add <owner/repo> [--ref <branch|tag|sha>]
ada skill update [<owner/repo>]
ada skill list
```

Run `ada package --help` or `ada skill --help` for the full flag list.
