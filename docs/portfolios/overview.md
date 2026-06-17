# Adaptive App Capability Portfolios

Delivering a consistent set of platform capabilities across cloud and edge environments is challenging due to differences in software availability, compatibility, and resource constraints.

To enable predictable portability, Adaptive Apps introduce the concept of a **capability portfolio**, which is a well-defined set of capabilities that an environment must provide in order to host an Adaptive App.

An application targets a specific capability portfolio and can be deployed to any environment that implements that portfolio.

## Portfolio dependency chain

Each portfolio is delivered by the unified [`charts/adaptive-apps`](../../charts/adaptive-apps/) Helm chart, parametrized by a profile file at `charts/adaptive-apps/profiles/<portfolio>.yaml`. Capability composition is described conceptually as a chain of supersets (each higher tier includes everything from the lower tier); the non-AI and AI chains are kept linear (no diamond dependencies), and each `*-ai` portfolio layers AI capabilities on top of its non-AI peer:

```
min ── core ── ent
 │      │       │
min-ai  core-ai ent-ai
```

- `min` — baseline capabilities for resource-constrained edge environments.
- `core` — `min` plus service mesh and observability for production-grade workloads.
- `ent` — `core` plus enterprise extensions.
- `min-ai` — `min` plus AI capabilities (in-cluster `aiModel` via KAITO).
- `core-ai` — `core` plus AI capabilities.
- `ent-ai` — `ent` plus AI capabilities.

## Capability matrix

The following table summarizes capabilities offered by each of the portfolios. A capability is inherited when the portfolio's chart depends (directly or transitively) on a chart that provides it.

| Capability | Min | Core | Ent | Min-AI | Core-AI | Ent-AI |
|---|---|---|---|---|---|---|
| User authentication (OIDC) | Keycloak | Keycloak | Keycloak | Keycloak | Keycloak | Keycloak |
| Workload identity | Entra Managed Identity | Entra Managed Identity / SPIFFE | Entra Managed Identity / SPIFFE | Entra Managed Identity | Entra Managed Identity / SPIFFE | Entra Managed Identity / SPIFFE |
| Service-to-service auth | Claim-based | Claim-based & mTLS (Istio) | Claim-based & mTLS (Istio) | Claim-based | Claim-based & mTLS (Istio) | Claim-based & mTLS (Istio) |
| Service mesh | — | Istio | Istio | — | Istio | Istio |
| Observability | — | OpenTelemetry Collector, Prometheus, Zipkin | OpenTelemetry Collector, Prometheus, Zipkin | — | OpenTelemetry Collector, Prometheus, Zipkin | OpenTelemetry Collector, Prometheus, Zipkin |
| Policy enforcement | — | — | Azure Policy / OPA (opa-envoy-plugin) as Istio extensionProvider | — | — | Azure Policy / OPA (opa-envoy-plugin) as Istio extensionProvider |
| Agent guardrails (in-pod) | n/a | n/a | n/a | Optional (opt in per app) | Optional (opt in per app) | Recommended default — AGT sidecar via Radius |
| AI inference | Cloud-based (e.g. Azure OpenAI) | Cloud-based (e.g. Azure OpenAI) | Cloud-based (e.g. Azure OpenAI) | In-cluster (via KAITO) + cloud-based | In-cluster (via KAITO) + cloud-based | In-cluster (via KAITO) + cloud-based |
| VM Management | Gantry | Gantry | Gantry | Gantry | Gantry | Gantry |

The following table maps capabilities to product/OSS offerings in different environment

| Capability | Azure | Azure Arc | Azure Local | Other cloud / On-premises |
|--------|--------|--------|--------|--------|
| User authentication | Entra | Entra | Entra | KeyCloak |
| Workload identity | Entra Managed Identity | Entra Managed Identity | Entra Managed Identity | Entra Managed Identity | 
| Service-to-service auth | mTLS / SPIFFE / Entra token | mTLS / SPIFFE / Entra token | mTLS / SPIFFE / Entra token | mTLS / SPIFFE / Entra token |
| Service mesh | Istio (AKS extension) | Istio | Istio | Istio |
| Observability | OTEL | OTEL | OTEL | OTEL |
| Policy enforcement | Azure Policy | Azure Policy | Azure Policy | OPA |
| Agent guardrails | [Agent Governance Toolkit](https://microsoft.github.io/agent-governance-toolkit/) sidecar (Microsoft-published [`ghcr.io/microsoft/agentmesh/governance-sidecar`](https://github.com/microsoft/agent-governance-toolkit/pkgs/container/agentmesh%2Fgovernance-sidecar)) | AGT sidecar | AGT sidecar | AGT sidecar |
| AI Inference | Microsoft OpenAI/KAITO |Microsoft OpenAI/KAITO |Microsoft OpenAI/KAITO |Microsoft OpenAI/KAITO |
| VM Management | ARM | Gantry | Gantry | TBD | 

## Implementation notes

- **Keycloak** is installed by the `min` profile and is therefore present in every portfolio. An optional `customCert` value mounts a CA certificate into the Keycloak pod; `hostAliases` can inject host-to-IP entries for on-prem domain controllers.
- **Istio** is installed by the chart on local Kubernetes (gated by `features.istio.install`), or enabled as the AKS Istio add-on (`--set features.istio.install=false --set istio.namespace=aks-istio-system`). Strict `PeerAuthentication` is applied to enrolled namespaces.
- **Observability** (OpenTelemetry Collector, Prometheus, Zipkin) is enabled by the `core` and higher profiles via `features.observability.enabled`.
- **Policy enforcement** is provided in the `ent` and `ent-ai` profiles, which deploy OPA with the `opa-envoy-plugin` and register it with Istio as the `opa-ext-authz-grpc` `extensionProvider`. Applications opt in via `AuthorizationPolicy` resources with `action: CUSTOM`. See the [governance tutorial](../../tutorials/governance/README.md).
- **Agent guardrails** are an application-layer companion to policy enforcement, intended only for AI workloads. The [Agent Governance Toolkit](https://microsoft.github.io/agent-governance-toolkit/) sidecar runs inside the agent's own pod (over `localhost`, no service mesh involvement) and provides prompt-injection scanning and governed tool execution. Provisioned by the `Radius.Resources/agentGuardrails` recipe (see [`radius/recipes/agent-guardrails/`](../../radius/recipes/agent-guardrails/)) and injected into the agent pod by [`radius/app.bicep`](../../radius/app.bicep) when `--parameters enableAgentGuardrails=true`. The recipe defaults to the Microsoft-published [`ghcr.io/microsoft/agentmesh/governance-sidecar`](https://github.com/microsoft/agent-governance-toolkit/pkgs/container/agentmesh%2Fgovernance-sidecar) image. The marker flag `features.agentGuardrails.enabled` defaults to `true` only in `ent-ai`; other portfolios can still opt in per application. See [`docs/agents/guard-rail.md`](../agents/guard-rail.md) for the full design.
- **Workload identity** is provided through Radius recipes ([radius/recipes/workload-identity](../../radius/recipes/workload-identity)) rather than the portfolio chart itself, so apps can bind the appropriate implementation (Azure workload identity or a local no-op) at deployment time.
- **AI inference** is exposed through the `aiModel` Radius resource type ([radius/recipes/ai-agent](../../radius/recipes/ai-agent)). All portfolios can bind `aiModel` to an internet-hosted endpoint (e.g. Azure OpenAI). The `*-ai` portfolios additionally support binding `aiModel` to an in-cluster [KAITO](https://github.com/kaito-project/kaito) workspace for self-hosted inference.
