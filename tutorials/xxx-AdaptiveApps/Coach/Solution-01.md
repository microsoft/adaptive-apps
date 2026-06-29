# Challenge 01 - Prepare the Platforms - Coach's Guide

[< Previous Solution](./Solution-00.md) - **[Home](./README.md)** - [Next Solution >](./Solution-02.md)

## Notes & Guidance

- The goal of this challenge is purely *platform setup*. Teams provision and validate one or more target environments before any Radius work begins — the cluster and supporting Azure resources prepared here are reused by every later challenge.
- Radius supports any CNCF-conformant Kubernetes cluster. For a WTH event, **Azure Kubernetes Service (AKS)** is strongly recommended because it integrates cleanly with Azure Container Registry (ACR) and Key Vault — both of which are used by recipes in later challenges. Local clusters (kind/k3d) work but make the recipe/ACR work in later challenges harder to demonstrate. Teams may also target **Arc-enabled Kubernetes** or **Azure Local** if those environments are available.
- The team should share **one Kubernetes cluster per target environment** so that each environment is consistent across team members. Radius will be installed on each chosen cluster in Challenge 2.
- Typical blockers to watch for:
  - AKS node image pulls on first start-up can take 5–10 minutes before all nodes are `Ready`. Tell teams to wait rather than re-running commands.
  - ACR attach / AcrPull role assignment propagation can take a minute or two after `az aks update --attach-acr`.
  - Misconfigured OIDC / workload identity flags — these are easy to miss and hard to add retroactively. Make sure teams enable them at cluster creation time.
- Expected time to complete for a team: **30–60 minutes**, most of which is AKS provisioning. Coach should wait ~15 minutes of apparent inactivity before stepping in.

## Solution Guide

This challenge focuses on provisioning the target platforms. Walk teams through the stages below in order; do not let them start Challenge 2 until at least one cluster and all supporting resources are healthy and available. If the team will complete Challenge 5 during the hack, preparing a second environment now is optional but useful.

#### Prepare the Environment(s)

Pick **one or multiple** environments and follow the linked guide. Each leaves you with
`kubectl` pointed at a working cluster:

| Environment | Guide |
| --- | --- |
| Local k3s (via k3d) | [`common/prepare-k3s.md`](../../common/prepare-k3s.md) |
| Azure Kubernetes Service (AKS) | [`common/prepare-aks.md`](../../common/prepare-aks.md) |
| Azure Arc-enabled cluster | [`common/prepare-arc.md`](../../common/prepare-arc.md) |
| Azure Local | [`common/prepare-azure-local.md`](../../common/prepare-azure-local.md) |

If no Azure Local, Arc-enabled, k3d, or other Kubernetes target is available, coaches may optionally use **two AKS clusters** for workshop purposes:

| Logical environment | Physical cluster | Later Radius workspace | Later Radius environment |
|---|---|---|---|
| Local / edge-like | AKS cluster #1 | `ws-local-prod` | `env-local-prod` |
| Azure / cloud | AKS cluster #2 | `ws-azure-prod` | `env-azure-prod` |

This optional fallback is not architecturally equivalent to Azure Local. It is a practical way to preserve the portability learning objective: the same application model is deployed to two Radius environments while the environment and recipe layer owns the platform-specific behavior.

The AKS installation with the `wi-helper.sh` script might fail with `MissingSubscription` because the helper attempts a tenant-level provider lookup. If the AKS cluster exists and workload identity is enabled, coaches can continue and revisit credential registration during the Radius setup.

For Azure Local workshop steps and command flow, use [`common/prepare-azure-local.md`](../../common/prepare-azure-local.md) as the source of truth.

#### Install the portfolio chart

Before moving to Challenge 02, ensure the portfolio baseline is installed with Helm. This mirrors the **Install the portfolio chart** step in [`tutorials/getting-started/README.md`](../../getting-started/README.md), but is included here so coaches do not need to jump between guides.

Set the same variables used in getting-started and install the chart:

```bash
export PORTFOLIO=min          # or: core | ent | min-ai | core-ai | ent-ai
export RELEASE=$PORTFOLIO
export NAMESPACE=$PORTFOLIO

helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  -n $NAMESPACE --create-namespace
```

For AKS, use the managed Istio override form from getting-started. This applies to both AKS clusters when using the optional two-AKS fallback, including the cluster that represents the logical local environment.

```bash
export AKS_CONTEXT="<aks-context-name>"
kubectl config use-context "$AKS_CONTEXT"
kubectl config current-context

helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  --set features.istio.install=false \
  --set istio.namespace=aks-istio-system \
  -n $NAMESPACE --create-namespace
```

For non-AKS local clusters, use the command below and make sure you are in the right Kubernetes context:

```bash
export LOCAL_CONTEXT="<local-kubernetes-context>"
kubectl config use-context "$LOCAL_CONTEXT"
kubectl config current-context

helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  -n $NAMESPACE --create-namespace
```

Validate the release before continuing:

```bash
helm ls -A
kubectl get pods -n $NAMESPACE
```