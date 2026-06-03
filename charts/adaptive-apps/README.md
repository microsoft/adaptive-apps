# adaptive-apps (unified capability chart)

A single Helm chart that delivers all Adaptive Apps capability portfolios.
Capability stacks (identity, mesh, observability, governance, AI marker)
are toggled via `features.*` flags, and the six [`profiles/`](./profiles)
overlays realize the `min`, `core`, `ent`, `min-ai`, `core-ai`, `ent-ai`
portfolios.

## Feature flags

| Flag | Capabilities turned on |
| --- | --- |
| `features.identity.enabled` | Keycloak Deployment + PostgreSQL StatefulSet, OIDC ConfigMap, optional ingress |
| `features.istio.install` | Pre-install Job that runs `helm install istio-base/istiod` from `istio-release.storage.googleapis.com/charts` |
| `features.istio.mtls` | Post-install Job that applies mesh-wide STRICT `PeerAuthentication` |
| `features.observability.enabled` | OTel Collector + Prometheus + Zipkin Deployments |
| `features.governance.opa` | OPA Deployment with Envoy ext_authz plugin |
| `features.governance.istioExtAuthz` | Post-install Job that patches the Istio ConfigMap to register OPA as an `extensionProvider` (requires `governance.opa=true`) |
| `features.ai.enabled` | Marker only; AI workloads are deployed via Radius `aiModel` resource types — the chart adds no manifests for this |

Per-capability config (image tags, replica counts, OIDC endpoints, etc.)
lives in [values.yaml](./values.yaml) under the same field names used by
the legacy charts.

## Install

Pick a profile and install:

```bash
# Reproduce the legacy `core` portfolio
helm install core charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/core.yaml \
  -n core --create-namespace
```

Mix-and-match by stacking overrides — e.g. core + governance on,
observability off:

```bash
helm install custom charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/core.yaml \
  --set features.observability.enabled=false \
  --set features.governance.opa=true \
  --set features.governance.istioExtAuthz=true \
  -n custom --create-namespace
```

AKS (managed Istio add-on) — disable the in-chart Istio install and point
mTLS at the managed namespace:

```bash
helm install core charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/core.yaml \
  --set features.istio.install=false \
  --set istio.namespace=aks-istio-system \
  -n core --create-namespace
```

## Profile → flag mapping

| Profile | identity | istio.install | istio.mtls | observability | governance.opa | ai |
| --- | :-: | :-: | :-: | :-: | :-: | :-: |
| `min` | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| `core` | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ |
| `ent` | ✓ | ✓ | ✓ | ✓ | ✗ (off by default) | ✗ |
| `min-ai` | ✓ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `core-ai` | ✓ | ✓ | ✓ | ✓ | ✗ | ✓ |
| `ent-ai` | ✓ | ✓ | ✓ | ✓ | ✗ (off by default) | ✓ |

## Layout

```
charts/adaptive-apps/
├── Chart.yaml
├── values.yaml
├── README.md
├── profiles/
│   ├── min.yaml
│   ├── core.yaml
│   ├── ent.yaml
│   ├── min-ai.yaml
│   ├── core-ai.yaml
│   └── ent-ai.yaml
└── templates/
    ├── _helpers.tpl
    ├── identity/        # Keycloak + PostgreSQL + OIDC ConfigMap + ingress
    ├── mesh/            # Istio install + STRICT PeerAuthentication
    ├── observability/   # OTel + Prometheus + Zipkin
    └── governance/      # OPA + Istio extensionProvider patch
```

## Notes

* `_helpers.tpl` exposes `min.*` / `core.*` / `ent.*` named-template
  aliases pointing at the shared `adaptive.*` helpers, so identity / mesh /
  governance templates can be referenced under either name.
* The `features.ai.enabled` flag exists for downstream tooling (CLI,
  Radius recipes) to detect intent; no manifests are gated on it inside
  the chart today.
