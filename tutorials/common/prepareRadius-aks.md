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
rad install kubernetes --set rp.publicEndpointOverride=localhost:8081
```

This deploys the Radius control plane (applications-rp, controller, bicep-de, UCP, dashboard, etc.) into the `radius-system` namespace.

### 1.3 Verify all Radius pods are healthy

```bash
kubectl get pods -n radius-system
kubectl get crds | grep radapp.io
```

Expected: All pods in `radius-system` should be `Running` or `Completed`. AKS node image pulls may take 5–10 minutes on first install — wait rather than re-running.

## Stage 2 — Configure workspaces and environments

### 2.1 Create workspace for AKS environment

```bash
rad workspace create kubernetes ws-azure-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-azure-prod
```

### 2.2 Create resource groups

```bash
rad group create rg-trading
rad group switch rg-trading
```

### 2.3 Create environment

```bash
rad env create env-azure-prod --group rg-trading --namespace prod
rad env switch env-azure-prod
```

### 2.4 Register Azure cloud provider

Store your subscription ID and resource group name, then:

```bash
rad env update env-azure-prod \
    --azure-subscription-id "$AZURE_SUBSCRIPTION" \
    --azure-resource-group "$RESOURCE_GROUP"
```

## Stage 3 — Validate setup

```bash
rad workspace list
rad env list
rad group list
```

Expected output:
- `ws-azure-prod` workspace active
- `env-azure-prod` environment listed with status `Succeeded`
- `rg-trading` resource group listed
- `env-azure-prod` should show Azure cloud provider registered

## Stage 4 — Register Azure credentials (optional but recommended)

The workload identity setup from [prepare-aks.md](./prepare-aks.md) created an Entra app for Radius. Bind it to the control plane:

```bash
export APPLICATION_CLIENT_ID=$(az ad app list \
  --query "[?displayName=='${AKS_CLUSTER}-radius-app'].appId | [0]" -o tsv)
export TENANT_ID=$(az account show --query tenantId -o tsv)

rad credential register azure wi \
  --client-id "$APPLICATION_CLIENT_ID" --tenant-id "$TENANT_ID"

# Verify (may take 30+ seconds to refresh):
rad credential show azure
```

## Stage 5 — Optional: Explore the dashboard

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open **http://localhost:7007** in a browser and verify:
- Environments tab shows `env-azure-prod` with Azure cloud provider
- Resource groups tab shows `rg-trading`
- Applications tab is empty (expected at this stage)

## Notes

- The `--azure-subscription-id` and `--azure-resource-group` allow recipes to provision Azure-managed resources (databases, storage, etc.) when applications are deployed.
- Workload identity integration allows applications running in the cluster to authenticate to Azure without sharing secrets.
- For production, ensure the Azure resource group has appropriate RBAC and network policies.

## Next steps

Proceed to Challenge 2 — Recipe authoring and application deployment.
