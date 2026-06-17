# Prepare an Azure Local AKS Cluster

Use this guide when your team is targeting Azure Local with AKS enabled by Azure Arc.

Azure Local runs Kubernetes via AKS enabled by Azure Arc. In this repo, the platform flag azure-local is passthrough: the bootstrap flow uses your current kubectl context and does not provision cloud resources for you.

Challenge 01 supports any CNCF-conformant cluster (AKS, kind, k3d, Arc-enabled Kubernetes, or Azure Local). If your team chooses AKS instead of Azure Local, use [prepare-aks](./prepare-aks.md) and enable OIDC issuer plus workload identity at cluster creation time.

## 0. Prerequisites

- An existing Azure Local environment with an AKS Arc cluster already provisioned by your platform owner.
- Network connectivity from your workstation to the cluster API endpoint.
- Azure CLI installed and signed in.
- kubectl installed.
- Helm installed.
- rad CLI installed.

If you do not already have a lab environment, you can provision one using Arc Jumpstart LocalBox:

- Getting started: https://jumpstart.azure.com/azure_jumpstart_localbox/getting_started
- AKS scenario: https://jumpstart.azure.com/azure_jumpstart_localbox/AKS

## 1. Set environment variables

Use consistent values across your terminal session:

```bash
export AZURE_SUBSCRIPTION=<your Azure subscription id>
export RESOURCE_GROUP=<resource group containing AKS Arc cluster>
export AKSARC_CLUSTER=<AKS Arc cluster name>
export AZURE_LOCATION=<Azure region, i.e. westus2>

# Use globally unique names where required
export ACR_NAME=<globally-unique-acr-name>
export KEYVAULT_NAME=<globally-unique-keyvault-name>
export STORAGE_ACCOUNT=<globally-unique-storage-account-name>
```

## 2. Configure Azure CLI and fetch cluster credentials

Azure CLI extension naming can vary by version. In most environments, aksarc is the required extension.

```bash
az login
az account set --subscription $AZURE_SUBSCRIPTION

az extension add --name aksarc --upgrade
az extension add --name connectedk8s --upgrade

# Optional: verify the cluster is visible before pulling kubeconfig
az aksarc show --resource-group $RESOURCE_GROUP --name $AKSARC_CLUSTER

# Merge credentials into your local kubeconfig
az aksarc get-credentials --resource-group $RESOURCE_GROUP --name $AKSARC_CLUSTER
```

Then start the Arc proxy in a dedicated terminal:

```bash
# Connect to the Arc-enabled Kubernetes API through a local proxy
az connectedk8s proxy -n $AKSARC_CLUSTER -g $RESOURCE_GROUP
```

Keep the proxy terminal open while working with the cluster. Open a second terminal for `kubectl`, `helm`, and `rad` commands.

### Optional: use the automation script

If you want one command that creates missing Challenge 01 resources and role assignments, run:

```powershell
.\tutorials\common\prepare-azure-local.ps1 -ResourceGroup jan-localbox-2604-adless-rg -ClusterName localbox-aks
```

If you also want it to create `acr-pull-secret` and patch/restart the test deployment (`acr-pull-check` in `default` namespace):

```powershell
.\tutorials\common\prepare-azure-local.ps1 -ResourceGroup jan-localbox-2604-adless-rg -ClusterName localbox-aks -ConfigureAcrPullSecret
```

After it runs, verify the test deployment reaches `Running`:

```bash
kubectl get pods -l app=acr-pull-check
```

Expected result (example):

```text
NAME                              READY   STATUS    RESTARTS   AGE
acr-pull-check-75cf98776f-5rxkz   1/1     Running   0          39s
```

This script automatically uses the same subscription and location as the target cluster.

Example output from a successful run:

```text
Using subscription: 608937df-4e8f-4dc5-8bc6-16f30646ebd9
Using location: australiaeast
ACR: janlocalbox2604adlessrglocalboxaksacr
Key Vault: janlocalbox2604adles-kv
Storage: janlocalbox2604adlessrgl
Created ACR.
Created Key Vault.
Created storage account.
Created role assignment: AcrPull
Created role assignment: Key Vault Secrets User
Created role assignment: Storage Blob Data Contributor

Completed. Resources and role assignments are in place.
```

## 3. Validate cluster access

Confirm your workstation is pointed to the right cluster and it is healthy:

```bash
kubectl config current-context
kubectl config get-contexts

kubectl cluster-info
kubectl get nodes -o wide
kubectl get pods -n kube-system
```

If nodes are not Ready yet, wait a few minutes and re-run checks before troubleshooting.

## 4. Provision Azure resources required by Challenge 01

Create the shared Azure resources used in later challenges:

```bash
az group create --name $RESOURCE_GROUP --location $AZURE_LOCATION

az acr create \
	--resource-group $RESOURCE_GROUP \
	--name $ACR_NAME \
	--sku Basic

az keyvault create \
	--resource-group $RESOURCE_GROUP \
	--name $KEYVAULT_NAME \
	--location $AZURE_LOCATION \
	--enable-rbac-authorization true

az storage account create \
	--resource-group $RESOURCE_GROUP \
	--name $STORAGE_ACCOUNT \
	--location $AZURE_LOCATION \
	--sku Standard_LRS
```

## 5. Grant cluster identity access (ACR, Key Vault, Storage)

Get the cluster principal id for role assignments. If this query returns empty, ask your platform owner for the principal id used by your Azure Local AKS Arc cluster.

```bash
export CLUSTER_PRINCIPAL_ID=$(az connectedk8s show \
	--resource-group $RESOURCE_GROUP \
	--name $AKSARC_CLUSTER \
	--query identity.principalId -o tsv)

export ACR_ID=$(az acr show --name $ACR_NAME --resource-group $RESOURCE_GROUP --query id -o tsv)
export KEYVAULT_ID=$(az keyvault show --name $KEYVAULT_NAME --resource-group $RESOURCE_GROUP --query id -o tsv)
export STORAGE_ID=$(az storage account show --name $STORAGE_ACCOUNT --resource-group $RESOURCE_GROUP --query id -o tsv)

# ACR pull permission for cluster workloads
az role assignment create \
	--assignee-object-id $CLUSTER_PRINCIPAL_ID \
	--assignee-principal-type ServicePrincipal \
	--role AcrPull \
	--scope $ACR_ID

# Key Vault RBAC data-plane access
az role assignment create \
	--assignee-object-id $CLUSTER_PRINCIPAL_ID \
	--assignee-principal-type ServicePrincipal \
	--role "Key Vault Secrets User" \
	--scope $KEYVAULT_ID

# Storage Blob data-plane access
az role assignment create \
	--assignee-object-id $CLUSTER_PRINCIPAL_ID \
	--assignee-principal-type ServicePrincipal \
	--role "Storage Blob Data Contributor" \
	--scope $STORAGE_ID
```

## 6. Verify Challenge 01 completion criteria

Verify healthy nodes from each team member workstation:

```bash
kubectl get nodes
```

Verify role assignments are in place:

```bash
az role assignment list --assignee-object-id $CLUSTER_PRINCIPAL_ID --scope $ACR_ID -o table
az role assignment list --assignee-object-id $CLUSTER_PRINCIPAL_ID --scope $KEYVAULT_ID -o table
az role assignment list --assignee-object-id $CLUSTER_PRINCIPAL_ID --scope $STORAGE_ID -o table
```

Optionally validate ACR pull with a test deployment:

```bash
# Import a public image to your ACR
az acr import --name $ACR_NAME --source docker.io/library/nginx:latest --image workshop/nginx:latest

# Deploy using the ACR image (no imagePullSecret)
cat <<EOF | kubectl apply -f -
apiVersion: apps/v1
kind: Deployment
metadata:
  name: acr-pull-check
spec:
  replicas: 1
  selector:
    matchLabels:
      app: acr-pull-check
  template:
    metadata:
      labels:
        app: acr-pull-check
    spec:
      containers:
      - name: nginx
        image: ${ACR_NAME}.azurecr.io/workshop/nginx:latest
EOF

kubectl get pods -l app=acr-pull-check
```

If the pod shows `ImagePullBackOff`, run these checks in order:

```bash
# 1) Inspect pull error details (auth vs not found)
kubectl describe pod -l app=acr-pull-check

# 2) Confirm the image tag exists in ACR
az acr repository show-tags --name $ACR_NAME --repository workshop/nginx -o table

# 3) Confirm AcrPull assignment exists for the cluster principal
az role assignment list \
	--assignee-object-id $CLUSTER_PRINCIPAL_ID \
	--scope $ACR_ID \
	--query "[?roleDefinitionName=='AcrPull'].{role:roleDefinitionName,scope:scope}" -o table
```

Common outcomes:

- `manifest unknown` or `not found`: the repo/tag path in your Deployment does not match the imported image. Re-import and redeploy.
- `unauthorized`: `AcrPull` is missing or still propagating. Wait 2 to 5 minutes and restart the pod.
- `unauthorized` with `AcrPull` already present: on some Azure Local / AKS Arc setups, node runtime may still not use that identity for ACR token exchange. Use an `imagePullSecret` for the validation deployment.

Managed identity note:

- Managed identity is still the preferred model for Azure API access from workloads.
- Image pull happens at node runtime before the container starts, so workload identity does not help with this specific pull path.
- On AKS Arc / Azure Local, if node-level ACR auth does not succeed even with `AcrPull`, use `imagePullSecret` for image pulls.

After fixing the issue, restart the test pod:

```bash
kubectl delete pod -l app=acr-pull-check
kubectl get pods -l app=acr-pull-check -w
```

If you need the `imagePullSecret` fallback:

```bash
# Create a pull secret from ACR admin credentials (for validation use)
az acr update --name $ACR_NAME --admin-enabled true
export ACR_USER=$(az acr credential show --name $ACR_NAME --query username -o tsv)
export ACR_PASS=$(az acr credential show --name $ACR_NAME --query passwords[0].value -o tsv)

kubectl create secret docker-registry acr-pull-secret \
	--docker-server=${ACR_NAME}.azurecr.io \
	--docker-username=$ACR_USER \
	--docker-password=$ACR_PASS

# Patch the test deployment to use the secret
kubectl patch deployment acr-pull-check --type='merge' -p '{"spec":{"template":{"spec":{"imagePullSecrets":[{"name":"acr-pull-secret"}]}}}}'
kubectl rollout restart deployment acr-pull-check
kubectl get pods -l app=acr-pull-check -w
```

After the test, rotate or disable ACR admin credentials if your policy requires it.

## 7. Known pitfalls

- Wrong subscription or tenant selected before running aksarc commands.
- Extension not installed or outdated, causing aksarc command failures.
- Multiple kubeconfig contexts, leading to commands running against the wrong cluster.
- Azure resource access works, but Kubernetes RBAC blocks kubectl actions.
- Firewall, proxy, or routing path from your workstation to the cluster API is not open.
- Role assignment propagation can take a few minutes before image pulls and data-plane calls succeed.

## 8. Continue with the Adaptive Apps tutorial

After kubectl validation succeeds, return to the getting started guide and continue with platform bootstrap using azure-local.

- Unified flow: [getting-started](../getting-started/README.md)
- Jump to section: [Bring up the platform](../getting-started/README.md#2-bring-up-the-platform)

For the CLI path, use platform azure-local in the bootstrap command. For the manual path, keep using the same kubectl context from this guide.

## References

- Azure Local documentation: https://learn.microsoft.com/azure/azure-local/
- AKS enabled by Azure Arc documentation: https://learn.microsoft.com/azure/aks/aksarc/
