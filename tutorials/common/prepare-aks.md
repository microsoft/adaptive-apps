# Prepare an AKS Cluster

## 0. Prerequisites

* An [Azure subscription](https://portal.azure.com/)
* [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/?view=azure-cli-latest) or Cloud Shell on Azure Portal
* [Helm](https://helm.sh/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)

## 1. Define a few environment variables for consistency

```bash
export AZURE_SUBSCRIPTION=<your Azure subscrption id>
export RESOURCE_GROUP=<Azure resource group>
export AZURE_LOCATION=<Azure region, i.e. westus2>
export AKS_CLUSTER=<AKS cluster name>
export RADIUS_WORKSPACE=aks-trading
export RADIUS_GROUP=trading
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
    --enable-addons monitoring \
    --generate-ssh-keys  \
    --enable-oidc-issuer \
    --enable-workload-identity
    ```
    > **NOTE:** If you want to reuse an existing AKS cluster, make sure OIDC issuer an Workload Idenity are enabled: `az aks update --resource-group <resource group name> --name <aks cluster name> --enable-oidc-issuer --enable-workload-identity`
    

4. Get AKS credential and merge to your `kubectl` config:
    ```bash
    az aks get-credentials --resource-group $RESOURCE_GROUP --name $AKS_CLUSTER
    ```
    > **NOTE:** The above command merges AKS cluster config into your local `kubectl` config. Run it from where you plan to use `kubectl` command.

##. Register Azure credentials 

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
    cd 
    ```