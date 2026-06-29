# Prepare Radius on Azure Arc-enabled Kubernetes

## Prerequisites

- Azure Arc-enabled Kubernetes cluster from [prepare-arc.md](./prepare-arc.md) with `kubectl` context active.
- `cluster-admin` access on the Arc cluster.
- `rad` CLI not yet installed (or version will be auto-updated).
- Azure subscription ID and resource group name available (for cloud provider registration).

## Stage 1 — Install Radius control plane

### 1.1 Install the rad CLI

```bash
wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
rad version
```

### 1.2 Install Radius into the Arc cluster

With the Arc-enabled cluster as the current `kubectl` context:

```bash
rad install kubernetes
```

This deploys the Radius control plane (applications-rp, controller, bicep-de, UCP, dashboard, etc.) into the `radius-system` namespace.

If this fails with `response status code 403: denied` while downloading the Radius Helm chart from `ghcr.io`, clear stale GitHub Container Registry credentials and retry:

```bash
helm registry logout ghcr.io || true
docker logout ghcr.io || true
rad install kubernetes
```

### 1.3 Verify all Radius pods are healthy

```bash
kubectl get pods -n radius-system
kubectl get crds | grep radapp.io
```

Expected: All pods in `radius-system` should be `Running` or `Completed`.

## Stage 2 — Configure workspaces and environments

### 2.1 Create workspace for Arc environment

```bash
rad workspace create kubernetes ws-local-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-prod
```

### 2.2 Create resource groups

```bash
rad group create rg-trading
rad group switch rg-trading
```

### 2.3 Create environment

```bash
rad env create env-local-prod --group rg-trading --namespace prod
rad env switch env-local-prod
```

### 2.4 (Optional) Register Azure cloud provider

If your Arc cluster has network access to Azure services and you want recipes to provision managed resources:

```bash
export AZURE_SUBSCRIPTION="<your-subscription-id>"
export RESOURCE_GROUP="<your-azure-resource-group>"

rad env update env-local-prod \
    --azure-subscription-id "$AZURE_SUBSCRIPTION" \
    --azure-resource-group "$RESOURCE_GROUP"
```

For disconnected or air-gapped scenarios, skip this step and keep recipes in-cluster only.

## Stage 3 — Validate setup

```bash
rad workspace list
rad env list
rad group list
```

Expected output:
- `ws-local-prod` workspace active
- `env-local-prod` environment listed with status `Succeeded`
- `rg-trading` resource group listed

## Stage 4 — Optional: Explore the dashboard

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open **http://localhost:7007** in a browser and verify:
- Environments tab shows `env-local-prod`
- Resource groups tab shows `rg-trading`
- Applications tab is empty (expected at this stage)
- Cloud provider registration status (if configured)

## Notes

- Arc-enabled clusters can be on-premises, edge, or multi-cloud. Radius works with any of these topologies.
- For disconnected/air-gapped scenarios, keep recipes in-cluster (do not register Azure cloud provider) and use local container registries.
- If you need to support multiple environments (prod and nonprod), create additional environments with different namespaces as needed.

## Next steps

Proceed to Challenge 2 — Recipe authoring and application deployment.
