# ent Capability Portfolio Chart

The `ent` portfolio builds on `core` and adds enterprise-grade policy
enforcement to the service mesh. The capability is a centralized
[Open Policy Agent (OPA)](https://www.openpolicyagent.org/) deployment
running the [opa-envoy-plugin](https://github.com/open-policy-agent/opa-envoy-plugin),
wired into Istio as a mesh `extensionProvider` of type `envoyExtAuthzGrpc`.

> **Migration notice.** As of this release the OPA workload and the Istio
> `extensionProvider` registration have moved to the Radius
> [`Radius.Resources/governance`](../../../radius/resource-types/types.yaml)
> resource type, materialized by the
> [`kubernetes-opa.bicep`](../../../radius/recipes/governance/kubernetes-opa.bicep)
> recipe. The chart-managed equivalents (`opa.enabled`,
> `istioExtAuthz.enabled`) are now **disabled by default**. Enable the
> `governance` resource in your app (set `enableGovernance=true` on
> `app.bicep`) to provision an application-scoped PDP, or set
> `opa.enabled=true` here to keep a chart-managed, cluster-wide OPA.

## Portfolio chart

- Chart path: `charts/portfolios/ent`
- Chart name: `ent`

## Dependency chain

- `ent` -> `core` -> `min`

## Capabilities

| Capability | Local cluster | AKS |
|------------|---------------|-----|
| Policy decision point (OPA + Envoy ext_authz plugin) | Installed by this chart as a Deployment + Service in the release namespace | Same |
| Istio external authorization integration | Post-install hook patches the `istio` ConfigMap to register OPA as `extensionProviders[name=opa-ext-authz-grpc]` and restarts istiod | Post-install hook patches the same ConfigMap; the AKS Istio add-on reconciles user customizations |
| Default policy bundle | `default allow := true` ConfigMap (override in production) | Same |

## Install

### Local Kubernetes (k3d / k3s)

Istio control plane is installed by the `core` chart dependency. OPA is
installed and registered with Istio automatically.

```bash
helm dependency update ./charts/portfolios/ent
helm install ent ./charts/portfolios/ent --namespace ent --create-namespace
```

Verify OPA is running and Istio has the extension provider registered:

```bash
kubectl get pods -n ent -l app=opa
kubectl -n istio-system get configmap istio -o jsonpath='{.data.mesh}' | grep -A4 extensionProviders
```

### AKS — control plane managed by AKS mesh add-on

Enable the AKS Istio add-on and install `core` with `istio.install.enabled=false`
(see [core/README.md](../core/README.md)), then install `ent`:

```bash
helm dependency update ./charts/portfolios/ent
helm install ent ./charts/portfolios/ent \
  --namespace ent --create-namespace \
  --set core.istio.install.enabled=false
```

## Configuration

| Value | Default | Description |
|-------|---------|-------------|
| `opa.enabled` | `false` | Install chart-managed OPA in the release namespace. Disabled by default — prefer the Radius `governance` recipe (per-app PDP) over a chart-managed cluster-wide PDP. |
| `opa.image.repository` | `openpolicyagent/opa` | OPA image repository. |
| `opa.image.tag` | `1.10.0-envoy` | OPA image tag. The `-envoy` suffix includes the Envoy ext_authz gRPC plugin. |
| `opa.grpcPort` | `9191` | Port served by the Envoy ext_authz plugin. Must match `istioExtAuthz` wiring. |
| `opa.httpPort` | `8181` | OPA management REST API port. |
| `opa.defaultDecisionPath` | `/envoy/authz/allow` | Rego rule consulted for each request. |
| `istioExtAuthz.enabled` | `false` | Register chart-managed OPA with Istio as an `extensionProvider`. Only meaningful when `opa.enabled=true`. |
| `istioExtAuthz.istioNamespace` | `istio-system` | Namespace containing the Istio MeshConfig. |
| `istioExtAuthz.istioConfigMap` | `istio` | Name of the ConfigMap holding the MeshConfig. |
| `istioExtAuthz.providerName` | `opa-ext-authz-grpc` | Provider name referenced from `AuthorizationPolicy.provider.name`. |

## Using OPA from an AuthorizationPolicy

Apply a `CUSTOM` AuthorizationPolicy that targets the registered provider.
This example sends all inbound requests to workloads labeled `app=frontend`
in the `myapp` namespace through OPA:

```yaml
apiVersion: security.istio.io/v1
kind: AuthorizationPolicy
metadata:
  name: opa-frontend
  namespace: myapp
spec:
  selector:
    matchLabels:
      app: frontend
  action: CUSTOM
  provider:
    name: opa-ext-authz-grpc
  rules:
  - to:
    - operation:
        paths: ["/*"]
```

To replace the default-allow policy, edit the `opa-policy` ConfigMap in the
release namespace or replace it via your GitOps tooling. OPA reloads
policies from disk automatically.
