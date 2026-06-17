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
| Local k3s (via k3d) | [`common/prepare-k3s.md`](../common/prepare-k3s.md) |
| Azure Kubernetes Service (AKS) | [`common/prepare-aks.md`](../common/prepare-aks.md) |
| Azure Arc-enabled cluster | [`common/prepare-arc.md`](../common/prepare-arc.md) |
| Azure Local | [`common/prepare-azure-local.md`](../common/prepare-azure-local.md) |

For Azure Local workshop steps and command flow, use [`common/prepare-azure-local.md`](../common/prepare-azure-local.md) as the source of truth.