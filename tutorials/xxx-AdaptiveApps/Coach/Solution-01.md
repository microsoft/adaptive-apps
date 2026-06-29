# Challenge 01 - Prepare the Platforms - Coach's Guide

[< Previous Solution](./Solution-00.md) - **[Home](./README.md)** - [Next Solution >](./Solution-02.md)

## Notes & Guidance

- The goal of this challenge is purely *platform setup*. Teams provision and validate one or more target environments before any Radius work begins — the cluster and supporting Azure resources prepared here are reused by every later challenge.
- Radius supports any CNCF-conformant Kubernetes cluster. For a WTH event, **Azure Kubernetes Service (AKS)** is strongly recommended because it integrates cleanly with Azure Container Registry (ACR) and Key Vault — both of which are used by recipes in later challenges. Local clusters (kind/k3d) work but make the recipe/ACR work in later challenges harder to demonstrate. Teams may also target **Arc-enabled Kubernetes** or **Azure Local** if those environments are available.
- The team should share **one** Kubernetes cluster so that the environment is consistent across team members. Radius will be installed on this cluster in Challenge 2.
- Typical blockers to watch for:
  - AKS node image pulls on first start-up can take 5–10 minutes before all nodes are `Ready`. Tell teams to wait rather than re-running commands.
  - ACR attach / AcrPull role assignment propagation can take a minute or two after `az aks update --attach-acr`.
  - Misconfigured OIDC / workload identity flags — these are easy to miss and hard to add retroactively. Make sure teams enable them at cluster creation time.
- Expected time to complete for a team: **30–60 minutes**, most of which is AKS provisioning. Coach should wait ~15 minutes of apparent inactivity before stepping in.

## Solution Guide

This challenge focuses on provisioning the target platforms. Walk teams through the stages below in order; do not let them start Challenge 2 until at least AKS cluster and all supporting resources are healthy and available. Optionally teams can deploy additional environments.

#### Prepare the Environment(s)

Pick **one or mutiple** environments and follow the linked guide. Each leaves you with
`kubectl` pointed at a working cluster:

| Environment | Guide |
| --- | --- |
| Local k3s (via k3d) | [`common/prepare-k3s.md`](../../common/prepare-k3s.md) |
| Azure Kubernetes Service (AKS) | [`common/prepare-aks.md`](../../common/prepare-aks.md) |
| Azure Arc-enabled cluster | [`common/prepare-arc.md`](../../common/prepare-arc.md) |
| Azure Local | [`common/prepare-azure-local.md`](../../common/prepare-azure-local.md) |

The AKS installation with the WI_helper script might fail "(MissingSubscription) The request did not have a subscription or a valid tenant level resource provider.
Code: MissingSubscription
Message: The request did not have a subscription or a valid tenant level resource provider." You can continue.

For Azure Local workshop steps and command flow, use [`common/prepare-azure-local.md`](../../common/prepare-azure-local.md) as the source of truth.

#### 2.2.2 Install the portfolio chart

Before moving to Challenge 02, ensure the portfolio baseline is installed with Helm. Reuse the same commands and options from [`tutorials/getting-started/README.md`](../../getting-started/README.md) section **2.2.2 Install the portfolio chart**.

Set the same variables used in getting-started and install the chart:

```bash
export PORTFOLIO=min          # or: core | ent | min-ai | core-ai | ent-ai
export RELEASE=$PORTFOLIO
export NAMESPACE=$PORTFOLIO
```
```bash
helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  -n $NAMESPACE --create-namespace
```

For AKS, use the managed Istio override form from getting-started:
!!! and make sure you are in the right AKS context
!!! if you are using AKS on Azure to virtualize the LOCAL environment use this variant also. !!!

```bash
kubectl config current-context

kubectl config use-context $AKS_CLUSTER$


helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  --set features.istio.install=false \
  --set istio.namespace=aks-istio-system \
  -n $NAMESPACE --create-namespace
```

For LOCAL, use the command below and make sure you are in the right AKS context

```bash
kubectl config current-context

kubectl config use-context $AKS_CLUSTER$

helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  -n $NAMESPACE --create-namespace
```

Validate the release before continuing:

```bash
helm ls -A
kubectl get pods -n $NAMESPACE
```
