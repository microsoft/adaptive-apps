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
    --network-plugin azure \
    --network-dataplane cilium \
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

5. Verify the portable-agent storage and network prerequisites:
    ```bash
    az aks show \
      --resource-group $RESOURCE_GROUP \
      --name $AKS_CLUSTER \
      --query '{networkPlugin:networkProfile.networkPlugin,networkDataplane:networkProfile.networkDataplane,networkPolicy:networkProfile.networkPolicy}' \
      -o table

    kubectl get storageclass \
      -o custom-columns='NAME:.metadata.name,DEFAULT:.metadata.annotations.storageclass\.kubernetes\.io/is-default-class,PROVISIONER:.provisioner'
    ```

    The network data plane must be `cilium`, and at least one dynamically
    provisioning storage class must be marked as the default. Do not deploy the
    integrated portable-agent path to an existing AKS cluster until both checks
    pass.

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

    Use `wi-helper.sh` script under the `tutorials/getting-started/assets/` folder to create an Entra application and service principal and set up service account federation:

    >**NOTE:** See https://docs.radapp.io/guides/operations/providers/azure-provider/howto-azure-provider-wi/ for more information

    ```bash
    ./wi-helper.sh $AKS_CLUSTER $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER
    ```

### Windows PowerShell with tenant-specific federation policy

For tenants requiring the trusted-issuer Graph API used by
[`wi-helper-private.sh`](../getting-started/assets/wi-helper-private.sh), use
[`wi-helper-private.ps1`](../getting-started/assets/wi-helper-private.ps1) from
Windows PowerShell 5.1 or PowerShell 7 with native Windows Azure CLI.
Windows and WSL Azure CLI authentication contexts are separate.

Run the following from the repository root, replacing the placeholder values.
The Windows login must satisfy your organization's Conditional Access policy;
the helper does not bypass token-protection requirements.

```powershell
$TENANT_ID = "<your tenant ID>"
$AZURE_SUBSCRIPTION = "<your subscription ID>"
$RESOURCE_GROUP = "<your resource group>"
$AKS_CLUSTER = "<your AKS cluster name>"

az config set core.enable_broker_on_windows=true
az login --tenant $TENANT_ID
az account set --subscription $AZURE_SUBSCRIPTION

$AKS_OIDC_ISSUER = az aks show `
    --subscription $AZURE_SUBSCRIPTION `
    --resource-group $RESOURCE_GROUP `
    --name $AKS_CLUSTER `
    --query oidcIssuerProfile.issuerUrl -o tsv

.\tutorials\getting-started\assets\wi-helper-private.ps1 `
    -K8sClusterName $AKS_CLUSTER `
    -AzureResourceGroup $RESOURCE_GROUP `
    -AzureSubscriptionId $AZURE_SUBSCRIPTION `
    -OidcIssuerUrl $AKS_OIDC_ISSUER
```

Before running, verify the helper's hardcoded `AksFicIssuerId` against the
cluster's OIDC issuer URL using the mappings in the script, and confirm the
hardcoded service-management reference is approved for your team. Changing the
tenant used for login does not select a different trusted-issuer binding.

Like the Bash helper, the PowerShell helper reuses the first matching
`<cluster>-radius-app` application, deletes and recreates the four named Radius
federated credentials, creates a service principal if absent, and grants it
**Owner on the supplied resource group**. You need permissions to manage the
Entra application and assign that Azure role. Azure CLI failures stop the
PowerShell helper; partial changes are not rolled back.