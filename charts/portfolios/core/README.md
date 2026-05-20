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
| Sidecar injection | Per-namespace via the `istio-injection=enabled` label (applied by Radius or the operator when the app namespace is created) | Same |
| mTLS (strict) | Mesh-wide `PeerAuthentication` in `istio-system` applies to every injected workload | Same |

## Install

### Local Kubernetes (k3d / k3s)

Istio control plane is installed automatically via Helm dependencies.

```bash
helm dependency update ./charts/portfolios/core
helm install core ./charts/portfolios/core --namespace core --create-namespace
```

Verify Istio is running and the mesh-wide PeerAuthentication is in place:

```bash
kubectl get pods -n istio-system
kubectl get peerauthentication -n istio-system
```

### AKS — control plane managed by AKS mesh add-on

Enable the AKS Istio add-on before installing the chart:

```bash
az aks mesh enable --resource-group <resource-group> --name <cluster-name>
```

Then install the chart with Istio install disabled (the chart still applies
the mesh-wide `PeerAuthentication`):

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
| `istio.namespace` | `istio-system` | Namespace for the Istio control plane. Also where the mesh-wide `PeerAuthentication` is applied. |
| `istio.mtls.strict` | `true` | Apply a mesh-wide `PeerAuthentication` with `STRICT` mode. |

## Enabling sidecar injection on app namespaces

The chart no longer enumerates app namespaces. Apply the injection label to any
namespace that should participate in the mesh — typically done by Radius when
it creates the app namespace, or manually:

```bash
kubectl label namespace <app-namespace> istio-injection=enabled --overwrite
```

Workloads in labeled namespaces will be injected with the Istio sidecar and
inherit the mesh-wide STRICT mTLS policy.

