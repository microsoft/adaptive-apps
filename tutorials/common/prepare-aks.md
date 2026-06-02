# Prepare an AKS Cluster

> **Status:** stub. A consolidated AKS recipe (cluster creation, managed Istio
> add-on enablement, Radius install with workload identity, ACR pull) for the
> unified [getting-started](../getting-started/README.md) flow is **not yet
> written**. Contributions welcome.

## What you need before running the unified [getting-started](../getting-started/README.md) tutorial on AKS

1. An AKS cluster with the **managed Istio add-on** enabled (only required for
   the `core`, `ent`, `core-ai`, `ent-ai` portfolios).
2. `kubectl` configured against the AKS cluster (`az aks get-credentials ...`).
3. `rad` CLI installed and a Radius workspace pointing at the AKS context.

Once the cluster is reachable, return to the unified tutorial at
[**§2 Bring up the platform**](../getting-started/README.md#2-bring-up-the-platform).
Remember to pass the AKS-specific flags when running `helm install`:

```bash
helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  --set features.istio.install=false \
  --set istio.namespace=aks-istio-system \
  -n $NAMESPACE --create-namespace
```
