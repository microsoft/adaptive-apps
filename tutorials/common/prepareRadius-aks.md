# Prepare Radius on Azure Kubernetes Service (AKS)

## Prerequisites

- Healthy AKS cluster from [prepare-aks.md](./prepare-aks.md) with `kubectl` context active.
- `cluster-admin` access on the AKS cluster (use `az aks get-credentials --admin` if needed).
- `rad` CLI not yet installed (or version will be auto-updated).
- Azure subscription ID and resource group name available (for cloud provider registration).

## Stage 1 — Install Radius control plane

### 1.1 Install the rad CLI

```bash
wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
rad version
```

### 1.2 Install Radius into the AKS cluster

With the AKS cluster as the current `kubectl` context:

```bash
rad install kubernetes \
  --set rp.publicEndpointOverride=localhost:8081 \
  --set global.azureWorkloadIdentity.enabled=true
```

This deploys the Radius control plane (applications-rp, controller, bicep-de, UCP, dashboard, etc.) into the `radius-system` namespace.
The workload identity setting is required when the Radius Azure provider uses `rad credential register azure wi`; without it, the credential is stored but the control-plane pods cannot exchange workload identity tokens.

If this fails with `response status code 403: denied` while downloading the Radius Helm chart from `ghcr.io`, clear stale GitHub Container Registry credentials and retry:

```bash
helm registry logout ghcr.io || true
docker logout ghcr.io || true
rad install kubernetes \
  --set rp.publicEndpointOverride=localhost:8081 \
  --set global.azureWorkloadIdentity.enabled=true
```

### 1.3 Verify all Radius pods are healthy

```bash
kubectl get pods -n radius-system
kubectl get crds | grep radapp.io
```

Expected: All pods in `radius-system` should be `Running` or `Completed`. AKS node image pulls may take 5–10 minutes on first install — wait rather than re-running.

## Stage 2 — Configure workspaces and environments

Set the logical names for the AKS cluster you are configuring. For the normal Azure path, use the Azure values. For the optional two-AKS workshop fallback, run this guide once per cluster and use the local values for the AKS cluster that represents the logical local/edge environment.

| Logical role | `RADIUS_WORKSPACE` | `RADIUS_ENVIRONMENT` | `RADIUS_NAMESPACE` |
|---|---|---|---|
| Local / edge-like stand-in | `ws-local-prod` | `env-local-prod` | `env-local-prod` |
| Azure / cloud environment | `ws-azure-prod` | `env-azure-prod` | `env-azure-prod` |

```bash
export RADIUS_WORKSPACE=ws-azure-prod
export RADIUS_ENVIRONMENT=env-azure-prod
export RADIUS_NAMESPACE=env-azure-prod
export RADIUS_GROUP=rg-trading
```

Do not describe the optional local stand-in as Azure Local. It is still AKS; it is only used to provide a second independent Radius control plane for the workshop.

### 2.1 Create workspace for AKS environment

```bash
rad workspace create kubernetes "$RADIUS_WORKSPACE" \
    --context "$(kubectl config current-context)" --force
rad workspace switch "$RADIUS_WORKSPACE"
```

### 2.2 Create resource groups

```bash
rad group create "$RADIUS_GROUP"
rad group switch "$RADIUS_GROUP"
```

### 2.3 Create environment

```bash
rad env create "$RADIUS_ENVIRONMENT" --group "$RADIUS_GROUP" --kubernetes-namespace "$RADIUS_NAMESPACE"
rad env switch "$RADIUS_ENVIRONMENT"
```

### 2.4 Register Azure cloud provider

Store your subscription ID and resource group name, then:

```bash
export AZURE_SUBSCRIPTION="<your-subscription-id>"
export RESOURCE_GROUP="<your-azure-resource-group>"

rad env update "$RADIUS_ENVIRONMENT" \
rad env update env-azure-prod \
    --azure-subscription-id "$AZURE_SUBSCRIPTION" \
    --azure-resource-group "$RESOURCE_GROUP"
```

If this AKS cluster is acting as the logical local/edge environment and you do not want recipes in that environment to provision Azure-managed resources, skip the Azure provider update and register the local recipes in Challenge 04 by deploying `radius/local-env.bicep` to `env-local-prod`.

## Stage 3 — Validate setup

```bash
rad workspace list
rad env list
rad group list
```

Expected output:
- chosen workspace active (for example `ws-azure-prod` or `ws-local-prod`)
- chosen environment listed with status `Succeeded`
- `rg-trading` resource group listed
- Azure cloud provider registered for the Azure environment, if configured

## Stage 4 — Register Azure credentials (optional but recommended)

The workload identity setup from [prepare-aks.md](./prepare-aks.md) created an Entra app for Radius. Bind it to the control plane:

```bash
export APPLICATION_CLIENT_ID=$(az ad app list \
  --query "[?displayName=='${AKS_CLUSTER}-radius-app'].appId | [0]" -o tsv)
export TENANT_ID=$(az account show --query tenantId -o tsv)

rad credential register azure wi \
  --client-id "$RADIUS_APP_ID" --tenant-id "$TENANT_ID"

# Verify (may take 30+ seconds to refresh):
rad credential show azure
```

For the optional two-AKS workshop fallback, register credentials independently in each Radius workspace only when that environment will use Azure-backed recipes. The Azure workspace normally needs this. The local stand-in workspace may not need it if it only uses local/container recipes.

## Stage 5 — Optional: Explore the dashboard

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open **http://localhost:7007** in a browser and verify:
- Environments tab shows the chosen environment (for example `env-azure-prod` or `env-local-prod`)
- Resource groups tab shows the chosen resource group (for example `rg-trading`)
- Applications tab is empty (expected at this stage)

## Notes

- The `--azure-subscription-id` and `--azure-resource-group` allow recipes to provision Azure-managed resources (databases, storage, etc.) when applications are deployed.
- Workload identity integration allows applications running in the cluster to authenticate to Azure without sharing secrets.
- In the optional two-AKS workshop fallback, both physical clusters are AKS, but they should still be treated as separate Radius sites with separate workspaces and environments.
- For production, ensure the Azure resource group has appropriate RBAC and network policies.

## Next steps

Proceed to Challenge 2 — Recipe authoring and application deployment.
