# Policies

Adaptive Apps treat policy as a cross-cutting concern that needs to be enforced at several different layers of the stack. This document explains which policy engine the platform standardises on, where it gets wired in, and how it composes with the other engines an organisation may already be running.

## Why a default policy engine?

There are many policy systems in production today, each optimised for a different layer:

| System                                                                                       | Primary concern                                                              |
| -------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| [Microsoft Purview](https://learn.microsoft.com/en-us/purview/purview)                       | Data governance — classification, lineage, DLP                              |
| [Kyverno](https://kyverno.io/)                                                               | Kubernetes-native admission control, validation, mutation                    |
| [Agent Governance Toolkit](https://github.com/microsoft/agent-governance-toolkit)            | Agent-level controls — prompt-injection scanning, governed tool execution    |
| [Open Policy Agent (OPA)](https://www.openpolicyagent.org/)                                  | General-purpose decision engine for application, gateway, and runtime checks |

Adaptive Apps adopt **OPA** as the default cross-cutting policy decision point. OPA's strengths — a unified policy language (Rego), a stateless request/response API, and broad ecosystem integration — make it well suited to authoring rules once and enforcing them at multiple points (the mesh, an LLM gateway, an admission controller, etc.). This choice does not preclude running the engines above in parallel; in practice most production estates layer several of them.

## Layers and enforcement points

The platform wires OPA into the layers where it provides the most value. Each row below is independently opt-in.

| Layer                  | Decision point                                                                                                       | Adaptive Apps wiring                                                                                                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Kubernetes admission   | [Gatekeeper](https://open-policy-agent.github.io/gatekeeper/website/) — OPA-as-admission-controller                  | On AKS: [Azure Policy for Kubernetes](https://learn.microsoft.com/en-us/azure/governance/policy/overview) (Gatekeeper-based). Off AKS: upstream Gatekeeper.            |
| Service mesh (L7)      | OPA + [`opa-envoy-plugin`](https://github.com/open-policy-agent/opa-envoy-plugin) registered as an Istio `extensionProvider` of kind `envoyExtAuthzGrpc`. Istio `AuthorizationPolicy` resources with `action: CUSTOM` delegate to the PDP. | `Radius.Resources/governance` recipe — [`kubernetes-opa.bicep`](../../radius/recipes/governance/kubernetes-opa.bicep) — enabled per app via `--parameters enableGovernance=true`. |
| AI gateway             | OPA decision evaluated by [LiteLLM](https://docs.litellm.ai/docs/) before forwarding a prompt to a model              | Reference integration only; bring your own LiteLLM deployment.                                                                                                       |
| AI agent (application) | In-pod [Agent Governance Toolkit](https://microsoft.github.io/agent-governance-toolkit/) sidecar — a complementary, content-aware layer for prompt-injection detection and governed tool calls. AGT is **not** OPA-based, but it shares the same defense-in-depth philosophy. | `Radius.Resources/agentGuardrails` recipe — see [`docs/agents/guard-rail.md`](../agents/guard-rail.md).                                                              |

## What's implemented in this repo

### 1. Resource type — `Radius.Resources/governance`

Defined in [`radius/resource-types/types.yaml`](../../radius/resource-types/types.yaml) alongside the rest of the custom Radius types. The type lets an application express intent ("deploy a PDP, in this mode, with this policy bundle, optionally wired into the mesh") and publishes the runtime coordinates (`decisionEndpoint`, provider name) that callers need to bind to it.

| Input                | Purpose                                                                                       |
| -------------------- | --------------------------------------------------------------------------------------------- |
| `mode`               | `enforce` (block), `audit` (log only), or `dryrun` (informational, reserved for future use)   |
| `policy`             | Inline Rego policy bundle loaded by OPA. When empty, a default-allow placeholder is loaded.   |
| `decisionPath`       | Rego rule the Envoy `ext_authz` plugin consults per request. Defaults to `/envoy/authz/allow`. |
| `istioIntegration`   | When `true`, also register the PDP as an Istio `extensionProvider` and roll istiod.            |
| `istioProviderName`  | Override the per-app provider name. Defaults to `opa-ext-authz-grpc-<resource>` to avoid collisions. |

### 2. Recipe — [`kubernetes-opa.bicep`](../../radius/recipes/governance/kubernetes-opa.bicep)

The Kubernetes-targeted recipe deploys a full PDP into the application namespace:

* `ServiceAccount`, `Deployment`, and `Service` for the `openpolicyagent/opa:<version>-envoy` image (the `-envoy` tag bundles the embedded `ext_authz` gRPC plugin).
* Two `ConfigMap`s — one for OPA runtime config (which enables the `envoy_ext_authz_grpc` plugin on the configured gRPC port), one for the Rego policy bundle.
* Liveness/readiness probes against the diagnostic endpoint so failed policy reloads surface as pod restarts.
* Optional post-install `Job` that patches the Istio `MeshConfig` `ConfigMap` to register the PDP as an `extensionProvider` and restarts `istiod`. This makes the recipe a drop-in replacement for the legacy OPA + post-install hook from the `ent` Helm portfolio.

The PDP pod is annotated with `sidecar.istio.io/inject: "false"` — it must not be subject to its own enforcement.

Why a recipe instead of a chart? Recipes make the PDP a first-class part of the application model: every app that opts into governance ships its own PDP and its own Rego, deployed by the same `rad deploy` invocation that brings up the rest of the app. The recipe also intentionally derives a **per-app** Istio `extensionProvider` name from the resource name, so two governed apps sharing a cluster cannot clobber each other's `MeshConfig.extensionProviders` entry.

### 3. Application wiring — [`radius/app.bicep`](../../radius/app.bicep)

When `enableGovernance=true` is passed to `rad deploy`, `app.bicep` provisions a `Radius.Resources/governance` resource named after the app and lets the recipe (with `istioIntegration=true` by default) handle the mesh wiring. Workloads then opt into enforcement by adding an Istio `AuthorizationPolicy` with `action: CUSTOM` and `provider.name` set to the per-app provider, exactly as they would for any other Istio ext_authz target.

The parameter defaults to `false` — governance is fully opt-in, even on the `ent`/`ent-ai` portfolios, so apps that don't need it pay no overhead.

## Portfolio defaults

The Helm chart marker `features.governance.opa` indicates the recommended posture per portfolio. The actual PDP is still per-application (it ships with the app via the Radius recipe), so the marker is purely informational — it tells operators which portfolios are expected to host governed workloads.

| Portfolio | `features.governance.opa` | Recommended posture                                              |
| --------- | :-----------------------: | ---------------------------------------------------------------- |
| `min`     |             —             | n/a — no Istio, no governance fabric                            |
| `core`    |             —             | n/a — native Istio `AuthorizationPolicy` only (no PDP)           |
| `ent`     |       Off by default      | **Opt-in** — enable per app via `--parameters enableGovernance=true` |
| `min-ai`  |             —             | n/a — no Istio                                                  |
| `core-ai` |             —             | n/a — native Istio `AuthorizationPolicy` only                    |
| `ent-ai`  |       Off by default      | **Opt-in** — typically paired with `enableAgentGuardrails=true`  |

See [`docs/portfolios/overview.md`](../portfolios/overview.md#features-by-portfolio) for the full capability matrix and [`tutorials/governance/README.md`](../../tutorials/governance/README.md) for an operator walkthrough.

## Enabling governance for a deployment

```bash
rad deploy radius/app.bicep \
  --group adaptive \
  --environment trading \
  ...other parameters... \
  --parameters enableGovernance=true \
  --parameters governanceMode=enforce
```

Optional overrides:

* `--parameters governanceIstioIntegration=false` — deploy the PDP but skip the Istio MeshConfig patch (useful when you wire `extensionProviders` out-of-band).
* Pass an inline Rego bundle by setting the `policy` property on the `governance` resource in your own Bicep (see [`radius/app.bicep`](../../radius/app.bicep) for the pattern).

To verify the PDP is live and answering decisions:

```bash
kubectl get pods -l app=opa -n trading-adaptive-apps
# expect: 1/1 Running

kubectl get cm istio -n istio-system -o jsonpath='{.data.mesh}' | grep -A2 extensionProviders
# expect: an entry named opa-ext-authz-grpc-<resource>
```

## Design notes and trade-offs

* **Defense in depth, not silver bullet.** The mesh-layer PDP answers "can this _request_ reach the service?" That is a coarse but cheap check. It does not see what an LLM is being _asked to do_ — that is the job of the [agent-guardrails layer](../agents/guard-rail.md). The two layers compose; on `ent-ai` we recommend running both.
* **Default-allow placeholder.** When `policy` is empty the recipe loads a `default allow := true` rule so the data plane stays functional out of the box. This is intentional for first-deploys; replace it before going to production.
* **Per-app provider names.** The recipe derives the Istio `extensionProvider` name from the resource name (`opa-ext-authz-grpc-<resource>`) so multiple governed apps in the same cluster never collide. Override only if you intentionally want apps to share a single PDP.
* **PDP outside its own mesh.** The PDP pod has `sidecar.istio.io/inject: "false"`. Subjecting the PDP to its own policies would risk locking the cluster out of the very engine that's supposed to unlock it.
* **Sidecar trust boundary.** Unlike the agent-guardrails sidecar, the OPA PDP is reached over a cluster-internal `Service`, not over `localhost`. mTLS comes from the surrounding Istio mesh; do not expose the PDP through a `Gateway`.

## See also

* [Open Policy Agent documentation](https://www.openpolicyagent.org/docs/latest/) — Rego language, deployment patterns, integrations
* [`opa-envoy-plugin`](https://github.com/open-policy-agent/opa-envoy-plugin) — the Envoy `ext_authz` gRPC plugin used by the recipe
* [`tutorials/governance/README.md`](../../tutorials/governance/README.md) — operator walkthrough for authoring `AuthorizationPolicy` (native + OPA-delegated)
* [`docs/agents/guard-rail.md`](../agents/guard-rail.md) — the application-layer companion for AI workloads
* [`docs/portfolios/overview.md`](../portfolios/overview.md) — which portfolios enable what
