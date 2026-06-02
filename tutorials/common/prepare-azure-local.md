# Prepare an Azure Local Cluster

> **Status:** stub. A consolidated Azure Local onboarding recipe for the unified
> [getting-started](../getting-started/README.md) flow is **not yet written**.
> Contributions welcome.

[Azure Local](https://learn.microsoft.com/en-us/azure/azure-local/) (formerly
Azure Stack HCI) runs Kubernetes via [AKS enabled by Azure
Arc](https://learn.microsoft.com/en-us/azure/aks/aksarc/). `ada bootstrap`
treats Azure Local as a passthrough platform — it does no cloud-side
automation and simply runs `helm`, `kubectl`, and `rad` against the
`kubectl` context you give it.

## What you need before running the unified [getting-started](../getting-started/README.md) tutorial on Azure Local

1. An Azure Local deployment with an AKS Arc cluster provisioned and
   reachable from your workstation.
2. `kubectl` configured against the cluster
   (`az aksarc get-credentials -g <rg> -n <cluster>`).
3. The cluster Arc-connected and visible in the Azure portal under your
   resource group.
4. `rad` CLI installed and a Radius workspace pointing at the cluster context.

Once your `kubectl` context points at the AKS Arc cluster, return to the
unified tutorial at
[**§2 Bring up the platform**](../getting-started/README.md#2-bring-up-the-platform)
and run `ada bootstrap` with `--platform azure-local` (or use the manual
OPTION 2 path).
