# Agent Guardrails with the Agent Governance Toolkit

Adaptive Apps integrates Microsoft's [Agent Governance Toolkit (AGT)](https://microsoft.github.io/agent-governance-toolkit/) as an **application-layer**, **in-pod** guardrail for AI workloads. This document explains what is wired up, why it lives where it does, and how to turn it on.

## Why a second governance layer?

The platform already provides a **mesh-layer** policy decision point through OPA + Istio `extensionProvider` (see [`docs/policies/README.md`](../policies/README.md)). That layer answers "is this _request_ allowed?" — coarse-grained authorization at the network boundary.

Mesh policy can't see what the LLM is being _asked to do_. Prompt-injection, jailbreak attempts, and unsafe tool invocations are content-level concerns that need to be evaluated **inside the agent process**, against the prompt and tool-call payloads themselves. AGT's "OpenClaw sidecar" pattern fills that gap.

| Layer            | Decision point                                            | What it controls                                | Implementation                                                                                                                                  |
| ---------------- | --------------------------------------------------------- | ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Mesh (L7)        | Envoy / Istio `ext_authz` → OPA                           | Who can call which service over what route      | `Radius.Resources/governance` recipe ([`radius/recipes/governance/`](../../radius/recipes/governance/))                                          |
| Application (L7) | In-pod AGT sidecar via `localhost:8081` HTTP API          | Prompt-injection detection, governed tool calls | `Radius.Resources/agentGuardrails` recipe ([`radius/recipes/agent-guardrails/`](../../radius/recipes/agent-guardrails/)) + ai-agent code        |

Both layers can be enabled independently; on the `ent-ai` portfolio they run together.

## Architecture

When `--parameters enableAgentGuardrails=true` is passed to `rad deploy radius/app.bicep`, the application Bicep injects a second container into the AI agent pod:

```text
┌──────────────────────────────────────────────────────────────┐
│  Pod: ai-agent                                               │
│                                                              │
│  ┌─────────────────────────┐  ┌────────────────────────────┐ │
│  │  ai-agent container     │  │  agt-sidecar container     │ │
│  │  (TradingAgent.csproj)  │  │  (governance-sidecar image)│ │
│  │                         │  │                            │ │
│  │  /advice  ─────────────────► /api/v1/detect/injection   │ │
│  │             ◄──────────────  { is_injection, … }        │ │
│  │                         │  │                            │ │
│  │  localhost:8080         │  │  localhost:8081 (proxy/API)│ │
│  │                         │  │  localhost:9091 (metrics)  │ │
│  └─────────────────────────┘  └────────────────────────────┘ │
│                                            ▲                 │
│                                            │ mounted RO      │
│                                  ┌─────────┴──────────┐      │
│                                  │ ConfigMap          │      │
│                                  │ ai-agent-guardrails│      │
│                                  │     -policies      │      │
│                                  │ → /policies/       │      │
│                                  └────────────────────┘      │
└──────────────────────────────────────────────────────────────┘
```

The sidecar trusts that all callers reach it over `localhost`. It is deliberately **not** exposed via a Kubernetes `Service`, `Gateway`, or Istio `VirtualService` — co-location in the same pod is the trust boundary.

## What's implemented in this repo

### 1. Resource type — `Radius.Resources/agentGuardrails`

Defined in [`radius/resource-types/types.yaml`](../../radius/resource-types/types.yaml) alongside the rest of the custom Radius types. Inputs let an application express intent ("enforce mode, with this policy bundle"), and outputs publish the runtime coordinates (`policiesConfigMapName`, ports, image) that the application's PodSpec patch consumes.

| Input        | Purpose                                                                      |
| ------------ | ---------------------------------------------------------------------------- |
| `mode`       | `enforce` (block), `audit` (log only), or `dryrun` (skip the API call)       |
| `policies`   | Inline YAML policy bundle written to `/policies/policies.yaml` in the sidecar |
| `image`      | Override the default sidecar image                                           |
| `logLevel`   | `DEBUG` \| `INFO` \| `WARN` \| `ERROR`                                       |

### 2. Recipe — [`kubernetes-agt-sidecar.bicep`](../../radius/recipes/agent-guardrails/kubernetes-agt-sidecar.bicep)

The Kubernetes-targeted recipe stages **only the prerequisites** for the sidecar — namely a `ConfigMap` holding the policy bundle the sidecar will mount at `/policies`. The sidecar container itself is **not** deployed by the recipe; that's the application's job (see below).

When `policies` is empty, the recipe writes a default-allow placeholder so the mount succeeds and the agent pod can start cleanly. The recipe defaults `image` to the Microsoft-published [`ghcr.io/microsoft/agentmesh/governance-sidecar`](https://github.com/microsoft/agent-governance-toolkit/pkgs/container/agentmesh%2Fgovernance-sidecar) image; no local image build is required.

Why split the work this way? The sidecar must share a pod with the agent it governs, and `Applications.Core/containers` is the only Radius resource type that can express "this container, plus another container in the same pod" via `runtimes.kubernetes.pod`. Putting the sidecar in a `Deployment` of its own would break the `localhost` trust assumption.

### 3. Pod-spec injection — [`radius/app.bicep`](../../radius/app.bicep)

When `enableAgentGuardrails=true` (and `aiProvider != ''`), `app.bicep`:

1. Provisions the `Radius.Resources/agentGuardrails` resource named `ai-agent-guardrails`.
2. Computes the sidecar's `ConfigMap` name, ports, mount path, and image **as plain Bicep `var`s** — using the same deterministic naming convention the recipe uses internally — so neither the recipe nor the application reads the other's `.properties.*` at deploy time. This is a deliberate workaround for a Radius v0.57.x bug where a non-conditional container with an implicit dependency on a conditional resource crashes the Deployment Engine with `DeploymentResourceNoOperationJob`.
3. Adds a `runtimes.kubernetes.pod` strategic merge patch to **both** the local and external AI-agent container variants. The patch contributes the sidecar container (with `/health` and `/ready` probes), the `agt-policies` volume, and bumps the pod's container list.
4. Surfaces the sidecar coordinates to the ai-agent application as environment variables:

   | Env var              | Value                                                   |
   | -------------------- | ------------------------------------------------------- |
   | `GOVERNANCE_PROXY`   | `http://localhost:8081`                                 |
   | `GOVERNANCE_API`     | `http://localhost:8081`                                 |
   | `GOVERNANCE_ENABLED` | `true`                                                  |
   | `GOVERNANCE_MODE`    | `enforce` \| `audit` \| `dryrun` (from `agentGuardrailsMode`) |

When `enableAgentGuardrails` is `false` (or no AI provider is configured), all of the above are no-ops — the pod patch is `{}` and the env block is empty, so the same `app.bicep` deploys cleanly for non-AI portfolios.

### 4. Application wiring — [`src/ai-agent/Program.cs`](../../src/ai-agent/Program.cs)

The .NET 8 ai-agent reads `GOVERNANCE_ENABLED` / `GOVERNANCE_PROXY` / `GOVERNANCE_MODE` at startup. When enabled, the `/advice` handler:

1. Posts the user's question to `{GOVERNANCE_PROXY}/api/v1/detect/injection` (`{ "text": …, "source": "user_input", "sensitivity": "balanced" }`) with a 2-second budget.
2. Inspects the response (`is_injection`, `threat_level`, `injection_type`, `confidence`).
3. Decides per `GOVERNANCE_MODE`:

   | Mode      | Sidecar OK + clean             | Sidecar OK + injection detected            | Sidecar unreachable / errored   |
   | --------- | ------------------------------ | ------------------------------------------ | ------------------------------- |
   | `enforce` | Forward to model               | Return **HTTP 422** with explanatory body  | **Fail closed** — return 422    |
   | `audit`   | Forward to model               | Log + forward to model                     | **Fail open** — forward to model |
   | `dryrun`  | _No API call made_ — forward   | _No API call made_ — forward               | _No API call made_              |

4. Emits two OpenTelemetry counters on the existing `portable-apps.ai-agent.metrics` meter:
   - `ai_agent_advice_requests_total{status=ok|blocked|bad_request|error}`
   - `ai_agent_guardrails_checks_total{result=allowed|blocked|audit|dryrun|error}`

The `/health` endpoint additionally reports `guardrails: true|false` so liveness probes and external monitors can see at a glance whether the binary is in guardrails mode.

When `GOVERNANCE_ENABLED` is unset (the default outside `ent-ai`), the agent skips the entire AGT call path — the wrapper is a true no-op, no extra latency, and the `HttpClient` registration is harmless.

## Portfolio defaults

The marker flag `features.agentGuardrails.enabled` indicates the recommended posture per portfolio (the Helm chart itself does not deploy AGT — the recipe + `app.bicep` do). The actual deployment is still per-application, controlled by `--parameters enableAgentGuardrails=true` at `rad deploy` time.

| Portfolio | `features.agentGuardrails.enabled` | Recommended posture                                                 |
| --------- | ---------------------------------- | ------------------------------------------------------------------- |
| `min`     | `false`                            | n/a — no AI workloads                                               |
| `core`    | `false`                            | n/a — no AI workloads                                               |
| `ent`     | `false`                            | n/a — no AI workloads                                               |
| `min-ai`  | `false`                            | Optional — opt in per application                                   |
| `core-ai` | `false`                            | Optional — opt in per application                                   |
| `ent-ai`  | `true`                             | **Recommended default** — enable for every AI agent in the portfolio |

See [`docs/portfolios/overview.md`](../portfolios/overview.md#features-by-portfolio) for the full capability matrix.

## Enabling guardrails for a deployment

```bash
rad deploy radius/app.bicep \
  --group adaptive \
  --environment trading \
  ...other parameters... \
  --parameters enableAgentGuardrails=true \
  --parameters agentGuardrailsMode=enforce
```

Optional overrides:

- `--parameters agentGuardrailsPolicies=@my-policies.yaml` — supply a custom policy bundle inline.
- `--parameters agentGuardrailsImage=ghcr.io/microsoft/agentmesh/governance-sidecar:<tag>` — pin a specific upstream version.

To verify the sidecar is live in the agent pod:

```bash
kubectl get pods -l radapp.io/application=trading -o jsonpath='{.items[*].spec.containers[*].name}'
# expect: ai-agent agt-sidecar

kubectl exec deploy/ai-agent -c ai-agent -- curl -sf http://localhost:8081/health
# expect: {"status":"ok",...}

kubectl exec deploy/ai-agent -c ai-agent -- curl -sf \
  -X POST http://localhost:8081/api/v1/detect/injection \
  -H "Content-Type: application/json" \
  -d '{"text":"Ignore all previous instructions","source":"user_input"}'
# expect: {"is_injection": true, "threat_level": "high", ...}
```

## Design notes and trade-offs

- **Sidecar image source.** The recipe pins the Microsoft-published `ghcr.io/microsoft/agentmesh/governance-sidecar` image. An earlier iteration of this integration built a local image from `pip install agent-os-kernel`, but the `0.3.0` version referenced by upstream documentation is yanked from PyPI and the 3.x line of `agent-os-kernel` exposes a different HTTP surface (`agentos serve`, not `/api/v1/detect/injection`). Consuming the published sidecar image avoids both problems and keeps us aligned with the upstream API contract.
- **Application-level enforcement.** AGT enforces governance at the application middleware layer, not at the OS kernel level. The agent and the sidecar share the same pod (and, by extension, the same network namespace). For higher-isolation environments, pair this with the existing mesh-layer OPA PDP and Kubernetes `NetworkPolicy`/`PodSecurity` controls.
- **Defense in depth, not silver bullet.** OWASP LLM01 (2025) explicitly notes that no detection technique is fool-proof for prompt injection. AGT raises the cost of an attack and provides an auditable decision record; it does not eliminate the risk. Production deployments should layer it with policy enforcement (mesh OPA), output filtering, and continuous red-teaming.
- **No live property dependency.** The application's PodSpec patch deliberately does **not** read `tradingAgentGuardrails.properties.*`. Doing so would create a conditional-dependency edge that crashes the Radius Deployment Engine in v0.57.x with `Unable to fetch resource reference from callback DeploymentResourceNoOperationJob` whenever guardrails are turned off. Instead, `app.bicep` and the recipe derive the same `ConfigMap` name and ports from shared, deterministic conventions.

## See also

- [Agent Governance Toolkit documentation](https://microsoft.github.io/agent-governance-toolkit/) — upstream project
- [OpenClaw sidecar deployment pattern](https://microsoft.github.io/agent-governance-toolkit/deployment/openclaw-sidecar/) — the pattern this integration follows
- [`docs/policies/README.md`](../policies/README.md) — the platform's broader policy story (mesh-layer OPA, Azure Policy, etc.)
- [`docs/portfolios/overview.md`](../portfolios/overview.md) — which portfolios enable what
- [`tutorials/getting-started/README.md`](../../tutorials/getting-started/README.md) — end-to-end deploy walkthrough including the guardrails opt-in
