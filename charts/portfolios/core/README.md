# core Capability Portfolio Chart

The `core` portfolio builds on `min` and adds enterprise-grade platform capabilities.
The first capability added is a service mesh (Istio) with strict mTLS between all services.

## Portfolio chart

- Chart path: `charts/portfolios/core`
- Chart name: `core`

## Dependency chain

- `core` -> `min`

## Capabilities

| Capability | Local cluster | AKS |
|------------|---------------|-----|
| Service mesh (Istio control plane) | Installed via chart (`istiod` + `base`) | AKS Istio add-on — not installed by this chart |
| Sidecar injection | Enabled per-namespace via `istio-injection=enabled` label | Same |
| mTLS (strict) | `PeerAuthentication` applied per namespace | Same |

## Install

### Local Kubernetes (k3d / k3s)

Istio control plane is installed automatically via Helm dependencies.

```bash
helm dependency update ./charts/portfolios/core
helm install core ./charts/portfolios/core --namespace core --create-namespace
```

Verify Istio is running:

```bash
kubectl get pods -n istio-system
kubectl get peerauthentication -n trading-portable-apps
```

### AKS — control plane managed by AKS mesh add-on

Enable the AKS Istio add-on before installing the chart:

```bash
az aks mesh enable --resource-group <resource-group> --name <cluster-name>
```

Then install the chart with Istio install disabled (the chart still applies
namespace enrollment and `PeerAuthentication`):

```bash
helm dependency update ./charts/portfolios/core
helm install core ./charts/portfolios/core \
  --namespace core --create-namespace \
  --set istio.install.enabled=false
```

## Configuration

| Value | Default | Description |
|-------|---------|-------------|
| `istio.install.enabled` | `true` | Install Istio via Helm. Set to `false` on AKS. |
| `istio.namespace` | `istio-system` | Namespace for the Istio control plane. |
| `istio.mtls.strict` | `true` | Apply `PeerAuthentication` with `STRICT` mode. |
| `istio.mtls.namespaces` | `[trading-portable-apps]` | App namespaces to enroll and protect. |

