# Prepare Radius on Azure Local

## Prerequisites

- Healthy Azure Local cluster from [prepare-azure-local.md](./prepare-azure-local.md) with `kubectl` context active.
- `cluster-admin` access on the Azure Local cluster.
- `rad` CLI not yet installed (or version will be auto-updated).
- Azure Local is typically fully disconnected or has limited/intermittent cloud connectivity.

## Stage 1 — Install Radius control plane

### 1.1 Install the rad CLI

```bash
wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
rad version
```

### 1.2 Install Radius into the Azure Local cluster

With the Azure Local cluster as the current `kubectl` context:

```bash
rad install kubernetes
```

This deploys the Radius control plane (applications-rp, controller, bicep-de, UCP, dashboard, etc.) into the `radius-system` namespace.

### 1.3 Verify all Radius pods are healthy

```bash
kubectl get pods -n radius-system
kubectl get crds | grep radapp.io
```

Expected: All pods in `radius-system` should be `Running` or `Completed`.

## Stage 2 — Configure workspaces and environments

### 2.1 Create workspace for Azure Local environment

For **connected** Azure Local:

```bash
rad workspace create kubernetes ws-local-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-prod
```

For **disconnected** Azure Local, use a distinct workspace name:

```bash
rad workspace create kubernetes ws-local-disconnected-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-disconnected-prod
```

### 2.2 Create resource groups

```bash
rad group create rg-finance
rad group create rg-hr
rad group switch rg-finance
```

### 2.3 Create environment

For connected Azure Local (with optional Azure backend):

```bash
rad env create env-local-prod --group rg-finance --namespace prod
rad env switch env-local-prod
```

For disconnected Azure Local (in-cluster recipes only):

```bash
rad env create env-local-disconnected-prod --group rg-finance --namespace prod-disconnected
rad env switch env-local-disconnected-prod
```

### 2.4 (Optional) Register Azure cloud provider (connected only)

Only if your Azure Local has reliable connectivity to Azure and you want cloud-based resources:

```bash
export AZURE_SUBSCRIPTION="<your-subscription-id>"
export RESOURCE_GROUP="<your-azure-resource-group>"

rad env update env-local-prod \
    --azure-subscription-id "$AZURE_SUBSCRIPTION" \
    --azure-resource-group "$RESOURCE_GROUP"
```

For disconnected scenarios, skip this step entirely.

## Stage 3 — Validate setup

```bash
rad workspace list
rad env list
rad group list
```

Expected output:
- Workspace active (`ws-local-prod` or `ws-local-disconnected-prod`)
- Environment listed with status `Succeeded`
- `rg-finance` and `rg-hr` resource groups listed

## Stage 4 — Optional: Explore the dashboard

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open **http://localhost:7007** in a browser and verify:
- Environments tab shows your environment
- Resource groups tab shows `rg-finance` and `rg-hr`
- Applications tab is empty (expected at this stage)
- Cloud provider registration status (if configured)

## Stage 5 — Data synchronization setup (optional, for hybrid scenarios)

If your Azure Local needs to sync data with cloud-based databases or services during connectivity windows, refer to [docs/data-sync/README.md](../../docs/data-sync/README.md) for platform-specific guidance (PostgreSQL, SQL Server, CDC patterns, etc.).

## Notes

- **Disconnected operation:** Azure Local is designed to operate independently without cloud connectivity. Configure in-cluster recipes only and handle data sync separately.
- **Naming convention:** Use `env-local-prod` for connected scenarios and `env-local-disconnected-prod` for offline scenarios so teams can distinguish them clearly.
- **Artifact mirroring:** For disconnected sites, mirror required container images and OCI artifacts to local registries before workload deployment.
- **Resilience:** Azure Local + federated Radius control planes (one per site) provide the strongest resilience for multi-site edge deployments.

## Next steps

1. If connected: Proceed to Challenge 2 — Recipe authoring and application deployment on Azure Local.
2. If disconnected: Proceed to Challenge 2, but keep all recipes in-cluster and set up data sync strategies per [docs/data-sync/README.md](../../docs/data-sync/README.md).
