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
