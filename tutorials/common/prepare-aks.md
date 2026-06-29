# Prepare an AKS Cluster

## 0. Prerequisites

* An [Azure subscription](https://portal.azure.com/)
* [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/?view=azure-cli-latest) or Cloud Shell on Azure Portal
* [Helm](https://helm.sh/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)

## 1. Define a few environment variables for consistency

Choose the values for the AKS cluster you are preparing:

```bash
export AZURE_SUBSCRIPTION=<your Azure subscrption id>
export RESOURCE_GROUP=<Azure resource group>
export AZURE_LOCATION=<Azure region, i.e. westus2>
export AKS_CLUSTER=<AKS cluster name>
export AKS_NODE_VM_SIZE=Standard_D2s_v5
export RADIUS_WORKSPACE=aks-trading
export RADIUS_GROUP=trading
export RADIUS_ENVIRONMENT=trading
```

### Optional workshop fallback: two AKS clusters

If this AKS cluster is one of two clusters used for the optional MicroHack portability fallback, run this guide once for each logical environment with distinct names:

| Logical role | Suggested variables |
|---|---|
| Local / edge-like stand-in | `AKS_CLUSTER=aks-local-prod`, `RADIUS_WORKSPACE=ws-local-prod`, `RADIUS_GROUP=rg-trading`, `RADIUS_ENVIRONMENT=env-local-prod` |
| Azure / cloud environment | `AKS_CLUSTER=aks-azure-prod`, `RADIUS_WORKSPACE=ws-azure-prod`, `RADIUS_GROUP=rg-trading`, `RADIUS_ENVIRONMENT=env-azure-prod` |

This optional lab shortcut is for events without Azure Local, Arc-enabled Kubernetes, k3d, or another second cluster type. It does **not** make AKS equivalent to Azure Local; it simply gives the team two separate Kubernetes clusters, kube contexts, Radius workspaces, and Radius environments for the portability exercises.

Before creating the second cluster, confirm quota for both AKS clusters in the target region and keep the `kubectl` contexts distinct:

```bash
kubectl config get-contexts
```

## 2. Prepare an Azure Kubernetes Service (AKS) cluster

1. Set the subscription you want to use:
    ```bash
    az login
    az account set --subscription $AZURE_SUBSCRIPTION
    ```

2. Create a resource group:
    ```bash
    az group create --name $RESOURCE_GROUP --location $AZURE_LOCATION
    ```

3. Create an AKS cluster:
    ```bash
    az aks create \
    --resource-group $RESOURCE_GROUP \
    --name $AKS_CLUSTER \
    --node-count 2 \
    --node-vm-size $AKS_NODE_VM_SIZE \
    --node-osdisk-type Managed \
    --enable-addons monitoring \
    --generate-ssh-keys  \
    --enable-oidc-issuer \
    --enable-workload-identity
    ```
    > **NOTE:** If you want to reuse an existing AKS cluster, make sure OIDC issuer an Workload Idenity are enabled: `az aks update --resource-group <resource group name> --name <aks cluster name> --enable-oidc-issuer --enable-workload-identity`

    > **Troubleshooting:** If cluster creation fails with `OverconstrainedAllocationRequest`, Azure could not allocate the requested node pool shape in that region. Keep `--node-osdisk-type Managed` to avoid ephemeral OS disk constraints, then retry with another available VM size such as `Standard_D4s_v5` or another nearby Azure region. If a failed cluster resource was partially created, delete it before retrying:
    >
    > ```bash
    > az aks delete --resource-group $RESOURCE_GROUP --name $AKS_CLUSTER --yes
    > ```
    

4. Get AKS credential and merge to your `kubectl` config:
    ```bash
    az aks get-credentials --resource-group $RESOURCE_GROUP --name $AKS_CLUSTER
    ```
    > **NOTE:** The above command merges AKS cluster config into your local `kubectl` config. Run it from where you plan to use `kubectl` command.

    If you choose the optional two-AKS fallback, verify that the active context matches the cluster you just prepared before continuing:

    ```bash
    kubectl config current-context
    kubectl get nodes
    ```

## 3. Register Azure credentials

1. You need the AKS OIDC issuer URL (also needed later for the app deploy):
    ```bash
    export AKS_OIDC_ISSUER=$(az aks show \
      --resource-group $RESOURCE_GROUP \
      --name $AKS_CLUSTER \
      --query oidcIssuerProfile.issuerUrl \
      -o tsv)
    ```

2. Register Azure credentials so Radius can provision Azure resources (Event Grid, Managed Identity, etc.):

    Use `wi-helper.sh` script under the `tutorials/getting-started/assets/` folder to create managed identity and set up service account federation:

    >**NOTE:** See https://docs.radapp.io/guides/operations/providers/azure-provider/howto-azure-provider-wi/ for more information

    ```bash
    cd tutorials/getting-started/assets
    ./wi-helper.sh "$AKS_CLUSTER" "$RESOURCE_GROUP" "$AZURE_SUBSCRIPTION" "$AKS_OIDC_ISSUER"
    ```

For the optional two-AKS workshop fallback, run the workload identity helper for each AKS cluster. Each cluster gets its own Radius workload identity and OIDC issuer, so do not reuse the `AKS_OIDC_ISSUER` value from the other cluster.