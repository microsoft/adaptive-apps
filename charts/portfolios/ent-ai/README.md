# ent-ai Capability Portfolio Chart

The `ent-ai` portfolio adds AI capabilities on top of `ent` (which already
provides Keycloak, Istio service mesh, observability, and OPA policy
enforcement via the Envoy ext_authz plugin).

## Portfolio chart

- Chart path: `charts/portfolios/ent-ai`
- Chart name: `ent-ai`

## Dependency chain

- `ent-ai` -> `ent` -> `core` -> `min`

## Capabilities

| Capability | Local cluster | AKS |
|------------|---------------|-----|
| Inherited from `ent` | Keycloak, Istio (self-managed), observability, OPA + Istio extensionProvider | Keycloak, Istio (AKS add-on), observability, OPA + Istio extensionProvider |
| In-cluster AI inference | Available via the `aiModel` Radius resource type bound to a KAITO recipe | Same |
| Cloud-hosted AI inference | Available via the `aiModel` Radius resource type bound to an Azure OpenAI recipe | Same |

AI capability is provided through the `aiModel` Radius resource type
([radius/recipes/ai-agent](../../../radius/recipes/ai-agent)); the chart
itself adds no additional Kubernetes manifests beyond what `ent` installs.

## Install

### Local Kubernetes (k3d / k3s)

```bash
helm dependency update ./charts/portfolios/ent-ai
helm install ent-ai ./charts/portfolios/ent-ai --namespace ent-ai --create-namespace
```

### AKS — control plane managed by AKS mesh add-on

```bash
helm dependency update ./charts/portfolios/ent-ai
helm install ent-ai ./charts/portfolios/ent-ai \
  --namespace ent-ai --create-namespace \
  --set ent.core.istio.install.enabled=false
```

## Configuration

All values are passed through to the `ent` subchart. See
[../ent/README.md](../ent/README.md) and [../core/README.md](../core/README.md)
for the full set. For example, to disable OPA while keeping the rest of the
stack:

```bash
helm install ent-ai ./charts/portfolios/ent-ai \
  --namespace ent-ai --create-namespace \
  --set ent.opa.enabled=false
```
