# Adaptive App Capability Portfolios

Delivering a consistent set of platform capabilities across cloud and edge environments is challenging due to differences in software availability, compatibility, and resource constraints.

To enable predictable portability, Adaptive Apps introduce the concept of a **capability portfolio**, which is a well-defined set of capabilities that an environment must provide in order to host an Adaptive App.

An application targets a specific capability portfolio and can be deployed to any environment that implements that portfolio.

## Portfolio dependency chain

Each portfolio is delivered as a Helm chart under [charts/portfolios](../../charts/portfolios/) and is composed by depending on a smaller portfolio. The non-AI and AI chains are kept linear (no diamond dependencies); each `*-ai` portfolio layers AI capabilities on top of its non-AI peer:

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
| User authentication (OIDC) | 🛡️ Keycloak | 🛡️ Keycloak | 🛡️ Keycloak | 🛡️ Keycloak | 🛡️ Keycloak | 🛡️ Keycloak |
| Identity datastore | PostgreSQL (in-cluster) | PostgreSQL (in-cluster) | PostgreSQL (in-cluster) | PostgreSQL (in-cluster) | PostgreSQL (in-cluster) | PostgreSQL (in-cluster) |
| Workload identity | Entra Managed Identity | Entra Managed Identity | Entra Managed Identity | Entra Managed Identity | Entra Managed Identity | Entra Managed Identity |
| Service-to-service auth | Claim-based | Claim-based & mTLS (Istio) | Claim-based & mTLS (Istio) | Claim-based | Claim-based & mTLS (Istio) | Claim-based & mTLS (Istio) |
| Service mesh | — | Istio (self-managed or AKS add-on) | Istio | — | Istio | Istio |
| Observability | — | OpenTelemetry Collector, Prometheus, Zipkin | OpenTelemetry Collector, Prometheus, Zipkin | — | OpenTelemetry Collector, Prometheus, Zipkin | OpenTelemetry Collector, Prometheus, Zipkin |
| AI inference | Cloud-based (e.g. Azure OpenAI) | Cloud-based (e.g. Azure OpenAI) | Cloud-based (e.g. Azure OpenAI) | In-cluster (via KAITO) + cloud-based | In-cluster (via KAITO) + cloud-based | In-cluster (via KAITO) + cloud-based |

Legend: 🛡️ identity/authentication capability · — not provided by this portfolio.

## Implementation notes

- **Keycloak** is installed by the `min` chart and is therefore present in every portfolio. An optional `customCert` value mounts a CA certificate into the Keycloak pod; `hostAliases` can inject host-to-IP entries for on-prem domain controllers. See [charts/portfolios/min/README.md](../../charts/portfolios/min/README.md).
- **Istio** is installed by the `core` chart via Helm on local Kubernetes, or enabled as the AKS Istio add-on (`istio.install.enabled=false`). Strict `PeerAuthentication` is applied to enrolled namespaces. See [charts/portfolios/core/README.md](../../charts/portfolios/core/README.md).
- **Observability** (OpenTelemetry Collector, Prometheus, Zipkin) is provisioned by the `core` chart and toggled via `observability.enabled`.
- **Workload identity** is provided through Radius recipes ([radius/recipes/workload-identity](../../radius/recipes/workload-identity)) rather than the portfolio chart itself, so apps can bind the appropriate implementation (Azure workload identity or a local no-op) at deployment time.
- **AI inference** is exposed through the `aiModel` Radius resource type ([radius/recipes/ai-agent](../../radius/recipes/ai-agent)). All portfolios can bind `aiModel` to an internet-hosted endpoint (e.g. Azure OpenAI). The `*-ai` portfolios additionally support binding `aiModel` to an in-cluster [KAITO](https://github.com/kaito-project/kaito) workspace for self-hosted inference.
