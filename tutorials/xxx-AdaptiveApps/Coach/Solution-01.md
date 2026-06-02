# Challenge 01 - Install and Configure the Radius Control Plane - Coach's Guide

[< Previous Solution](./Solution-00.md) - **[Home](./README.md)** - [Next Solution >](./Solution-02.md)

## Notes & Guidance

- The goal of this challenge is purely *platform setup*. Nothing application-related is deployed yet — the environment built here is reused by every later challenge (recipes, environments, app deployment). Resist the temptation to help teams "skip ahead" to deploying an app.
- Radius supports any CNCF-conformant Kubernetes cluster. For a WTH event, **Azure Kubernetes Service (AKS)** is strongly recommended because it integrates cleanly with Azure Container Registry (ACR), Key Vault, and Storage — all of which are used by recipes in later challenges. Local clusters (kind/k3d) work but make the recipe/ACR work in later challenges harder to demonstrate.
- Each team member installs the `rad` CLI on their own workstation, but the team should share **one** AKS cluster and **one** Radius control plane so that workspaces and environments are consistent across the team.
- The Radius control plane installs CRDs and cluster-scoped resources, so the user running `rad install kubernetes` must have `cluster-admin` on the AKS cluster. Using the AKS cluster admin credentials (`az aks get-credentials --admin`) is the simplest way for a coach to unblock teams.
- Typical blockers to watch for:
  - Mismatched `kubectl` context — teams sometimes install Radius into a leftover local cluster instead of their AKS cluster. Have them run `kubectl config current-context` before `rad install kubernetes`.
  - AKS node image pulls on first start-up can take 5–10 minutes before all Radius pods are `Ready`. Tell teams to go get coffee instead of re-running the installer.
  - `rad` CLI version mismatch with the control plane chart — always install the latest CLI **and** let `rad install kubernetes` pick the matching chart. Do not pin a chart version unless you have a reason to.
  - ACR attach / AcrPull role assignment propagation can take a minute or two. If pods in `radius-system` are in `ImagePullBackOff` against a private ACR, wait and retry before debugging.
- Expected time to complete for a team: **45–75 minutes**, most of which is AKS and supporting Azure resource provisioning. Coach should wait ~15 minutes of apparent inactivity before stepping in.

## Solution Guide

This challenge is naturally split into four stages. Walk teams through the stages in order; do not let them start stage 3 until stage 1 and 2 are healthy.

### Stage 1 — Provision the AKS cluster in Azure

- Create a resource group in the Azure region closest to the team.
- Create an AKS cluster with:
  - A system node pool of at least 2 nodes (3 recommended), VM size `Standard_D2s_v5` or larger.
  - Managed identity enabled (default).
  - OIDC issuer and workload identity enabled — Radius recipes in later challenges use workload identity to talk to Azure.
  - Azure CNI or kubenet (either works; Azure CNI preferred for later recipes that touch Azure networking).
- Fetch the kubeconfig for the cluster and set it as the current `kubectl` context.
- Sanity check: `kubectl get nodes` returns all nodes in `Ready`.

### Stage 2 — Prepare AKS for hosting Radius (keys, secrets, storage, ACR)

- Create an **Azure Container Registry (ACR)** that the team will use to push application images in later challenges, and attach it to the AKS cluster so it can pull images without a pull secret (`az aks update --attach-acr`).
- Create an **Azure Key Vault** for application secrets and for the certificates that Radius recipes will consume in later challenges. Enable RBAC authorization on the vault.
- Create an **Azure Storage account** that later recipes (blob, table, queue) will target. Use Standard LRS for hack purposes.
- Grant the AKS kubelet identity `AcrPull` on the ACR (this is done automatically by `--attach-acr`, but verify with `az role assignment list`).
- Grant the AKS cluster's workload identity (or a user-assigned managed identity you create for Radius recipes) the appropriate data-plane roles:
  - `Key Vault Secrets User` and `Key Vault Certificates User` on the Key Vault.
  - `Storage Blob Data Contributor` on the Storage account.
- Create a dedicated Kubernetes namespace for Radius (`radius-system` is the default used by the installer — do **not** pre-create this if you plan to use the default installer; let `rad install kubernetes` create it).
- Create a Kubernetes namespace the team will use for their application workloads in later challenges (for example `default` is fine, or `demo-app`).

### Stage 3 — Install the Radius control plane

- On each workstation, install the `rad` CLI from the Radius project install script and verify with `rad version`.
- With the AKS cluster as the current `kubectl` context, run `rad install kubernetes`. This deploys the Radius control plane (applications-rp, controller, bicep-de, UCP, dashboard, etc.) into the `radius-system` namespace.
- Verify:
  - `kubectl get pods -n radius-system` shows all pods `Running` / `Ready`.
  - `kubectl get crds | grep radapp.io` returns the Radius CRDs.
- Only one team member needs to run `rad install kubernetes` against the shared AKS cluster; everyone else reuses that install.

### Stage 4 — Post-install configuration (workspace + environment)

- On each workstation, create a Radius **workspace** pointing at the AKS cluster: `rad workspace create kubernetes <workspace-name> --context <aks-kubectl-context>` and make it current with `rad workspace switch`.
- Create a **resource group** inside Radius' own Universal Control Plane (UCP) — this is the Radius logical scope, not the Azure resource group — with `rad group create <name>` and switch to it.
- Create a default **environment** with `rad env create default --namespace <k8s-namespace>` (or let `rad init --full` drive the whole setup interactively — both are acceptable).
- If the team plans to use Azure recipes in later challenges, register the Azure cloud provider on the environment with `rad env update default --azure-subscription-id <sub> --azure-resource-group <rg>`.
- Sanity check:
  - `rad workspace list` shows the workspace marked as current.
  - `rad env list` shows `default` selected.
  - `rad group list` shows the UCP group.
- Stop here. Recipe authoring, environment customization, and app deployment are the next challenges — do not let teams get ahead of themselves.

## Sample deployment script

The script below provisions stage 1 and stage 2 (AKS + supporting Azure resources) end-to-end, then performs stage 3 (Radius control plane install) and a minimal stage 4 (workspace + environment). Hand it to teams only if they are stuck — the point of the challenge is for the students to work out the moving parts themselves.

(If you are not using Bash, prefix variables with `$` and double-quote values as appropriate.)

```bash
# ---------------------------------------------------------------------------
# Stage 1 — AKS cluster
# ---------------------------------------------------------------------------

# Variables (change as relevant)
rg=radiushack-rg
location=westeurope
aks_name=radius-aks
aks_node_count=3
aks_node_size=Standard_D2s_v5

# Unique suffix for globally-unique resource names (ACR, KV, storage)
suffix=$RANDOM$RANDOM
acr_name=radiushackacr${suffix}
kv_name=radiushack-kv-${suffix}
stg_name=radiushackstg${suffix}

# Radius environment
radius_namespace=default
radius_workspace=radius-hack
radius_group=radius-hack
radius_env=default

# Create resource group
echo "Creating resource group..."
az group create -n "$rg" -l "$location" -o none

# Create ACR (for app images used in later challenges)
echo "Creating Azure Container Registry..."
az acr create -g "$rg" -n "$acr_name" --sku Standard -o none

# Create AKS cluster with OIDC + workload identity and attach ACR
echo "Creating AKS cluster (this takes ~5-10 minutes)..."
az aks create \
    -g "$rg" -n "$aks_name" \
    --node-count "$aks_node_count" \
    --node-vm-size "$aks_node_size" \
    --enable-managed-identity \
    --enable-oidc-issuer \
    --enable-workload-identity \
    --network-plugin azure \
    --attach-acr "$acr_name" \
    --generate-ssh-keys \
    -o none

# Fetch kubeconfig and make it current
echo "Fetching kubeconfig..."
az aks get-credentials -g "$rg" -n "$aks_name" --overwrite-existing
kubectl get nodes

# ---------------------------------------------------------------------------
# Stage 2 — Prepare AKS: Key Vault, Storage, RBAC
# ---------------------------------------------------------------------------

echo "Creating Key Vault..."
az keyvault create -g "$rg" -n "$kv_name" -l "$location" \
    --enable-rbac-authorization true -o none

echo "Creating Storage account..."
az storage account create -g "$rg" -n "$stg_name" -l "$location" \
    --sku Standard_LRS --kind StorageV2 -o none

# Grant the AKS kubelet identity data-plane access to KV and Storage.
# (ACR AcrPull is already granted by --attach-acr above.)
kubelet_obj_id=$(az aks show -g "$rg" -n "$aks_name" \
    --query identityProfile.kubeletidentity.objectId -o tsv)
kv_id=$(az keyvault show -g "$rg" -n "$kv_name" --query id -o tsv)
stg_id=$(az storage account show -g "$rg" -n "$stg_name" --query id -o tsv)
sub_id=$(az account show --query id -o tsv)

az role assignment create --assignee-object-id "$kubelet_obj_id" \
    --assignee-principal-type ServicePrincipal \
    --role "Key Vault Secrets User" --scope "$kv_id" -o none
az role assignment create --assignee-object-id "$kubelet_obj_id" \
    --assignee-principal-type ServicePrincipal \
    --role "Storage Blob Data Contributor" --scope "$stg_id" -o none

# ---------------------------------------------------------------------------
# Stage 3 — Install the Radius control plane
# ---------------------------------------------------------------------------

# Install the rad CLI (Linux / macOS / WSL). On Windows use the PowerShell
# install script from https://docs.radapp.io/installation/ instead.
if ! command -v rad >/dev/null 2>&1; then
    echo "Installing rad CLI..."
    wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
fi
rad version

echo "Installing Radius control plane into AKS..."
rad install kubernetes

echo "Waiting for Radius pods to become Ready..."
kubectl wait --for=condition=Ready pods --all -n radius-system --timeout=10m
kubectl get pods -n radius-system

# ---------------------------------------------------------------------------
# Stage 4 — Post-install configuration: workspace, group, environment
# ---------------------------------------------------------------------------

aks_context=$(kubectl config current-context)

echo "Creating Radius workspace pointing at AKS..."
rad workspace create kubernetes "$radius_workspace" \
    --context "$aks_context" --force
rad workspace switch "$radius_workspace"

echo "Creating Radius resource group (UCP)..."
rad group create "$radius_group"
rad group switch "$radius_group"

echo "Creating default Radius environment..."
rad env create "$radius_env" --namespace "$radius_namespace"
rad env switch "$radius_env"

# Register the Azure cloud provider on the environment for Azure recipes
# used in later challenges.
rad env update "$radius_env" \
    --azure-subscription-id "$sub_id" \
    --azure-resource-group "$rg"

echo "Verifying Radius install..."
rad workspace list
rad group list
rad env list

echo "Done. AKS: $aks_name  ACR: $acr_name  KV: $kv_name  Storage: $stg_name"
```

After this script completes, the team should see their workspace, resource group, and `default` environment in the `rad` CLI and all pods in the `radius-system` namespace should be `Running`. They are now ready for the next challenge.
