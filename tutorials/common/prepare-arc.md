# Prepare an Azure Arc-enabled Cluster

> **Status:** stub. A consolidated Arc onboarding recipe for the unified
> [getting-started](../getting-started/README.md) flow is **not yet written**.
> Contributions welcome.

## What you need before running the unified [getting-started](../getting-started/README.md) tutorial on Azure Arc

1. A Kubernetes cluster (anywhere: on-prem, edge, another cloud) reachable from
   your workstation.
2. The cluster connected to Azure Arc (`az connectedk8s connect ...`).
3. `kubectl` configured against the Arc-connected cluster.
4. `rad` CLI installed and a Radius workspace pointing at the cluster context.

Once your cluster shows `Connected` under `az connectedk8s show`, return to the
unified tutorial at
[**§2 Bring up the platform**](../getting-started/README.md#2-bring-up-the-platform).
