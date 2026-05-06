# Deploy Adaptive App to Azure

## 0. Prerequisites

* An [Azure subscription](https://portal.azure.com/)
* [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/?view=azure-cli-latest) or Cloud Shell on Azure Portal
* [Helm](https://helm.sh/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
* [rad](https://docs.radapp.io/guides/tooling/rad-cli/howto-rad-cli/)
* (optional) An [OpenAI API Key](https://platform.openai.com/api-keys) or [Azure OpenAI deployment key](https://azure.microsoft.com/en-us/products/ai-foundry/models/openai)

## 1. Define a few environment variables for consistency

    ````bash
    export AZURE_SUBSCRIPTION=<your Azure subscrption id>
    export RESOURCE_GROUP=<Azure resource group>
    export AZURE_LOCATION=<Azure region, i.e. westus2>
    export AKS_CLUSTER=<AKS cluster name>
    export RADIUS_WORKSPACE=aks-trading
    export RADIUS_GROUP=trading
    ```
## 1. Prepare an Azure Kubernetes Service (AKS) cluster

For Azure deployment, you'll use AKS as the deployment target.

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

## 2. Set up Radius on AKS

Set up Radius on AKS and register the custom resource types used by the app model.

1. Verify the current Kubernetes context points to AKS:
    ```bash
    kubectl config current-context
    kubectl cluster-info
    ```

2. Install Radius control plane:
    ```bash
    rad install kubernetes --set global.azureWorkloadIdentity.enabled=true
    ```

3. Create and switch to a Radius workspace bound to this AKS context:
    ```bash
    rad workspace create kubernetes $RADIUS_WORKSPACE --context $AKS_CLUSTER --force
    rad workspace switch $RADIUS_WORKSPACE
    rad workspace show
    ```

4. You need the AKS OIDC issuer URL (also needed later for the app deploy):
    ```bash
    export AKS_OIDC_ISSUER=$(az aks show \
      --resource-group $RESOURCE_GROUP \
      --name $AKS_CLUSTER \
      --query oidcIssuerProfile.issuerUrl \
      -o tsv)
    ```

5. Register Azure credentials so Radius can provision Azure resources (Event Grid, Managed Identity, etc.):

    Use `wi-helper.sh` script under the `tutorials/min-getting-started/deploy-to-azure` folder to create managed identity and set up service account federation:

    >**NOTE:** See https://docs.radapp.io/guides/operations/providers/azure-provider/howto-azure-provider-wi/ for more information

    ```bash
    ./wi-helper.sh $AKS_CLUSTER $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER
    ```

4. Create the Radius group:
    ```bash
    rad group create $RADIUS_GROUP
    ```

5. Register custom resource types used by the sample app:
    ```bash
    cd radius
    rad resource-type create --from-file resource-types/types.yaml
    ```

6. Create the AKS Radius environment and recipe bindings:
    ```bash
    rad env create $RADIUS_WORKSPACE --group $RADIUS_GROUP
    rad deploy aks-env.bicep \
      --group $RADIUS_GROUP \
      --parameters azureSubscriptionId=$AZURE_SUBSCRIPTION \
      --parameters azureResourceGroup=$RESOURCE_GROUP

    rad environment list --group $RADIUS_GROUP
    ```
7. Register credential

    When you used above helper script, it created an application with name `<AKS cluster name>-radius-app`. Next, register the credential:

    ```bash
    export APPLICATION_NAME=$AKS_CLUSTER-radius-app
    export APPLICATION_CLIENT_ID="$(az ad app list --display-name "${APPLICATION_NAME}" --query [].appId -o tsv)"
    export TENANT_ID="$(az account show --query tenantId -o tsv)"
    ```

    rad credential register azure wi --client-id $APPLICATION_CLIENT_ID --tenant-id $TENANT_ID
    ```
    Verify credentials are registered (this may take 30+ seconds to refresh):
    ```bash
    rad credential show azure
    ```

## 3. Install Adaptive App Capability Portfolio

Start with the `min` portfolio Helm chart. The first bundled component is Keycloak.

1. Create namespace:
    ```bash
    kubectl create namespace min
    ```

2. Install the `min` portfolio from the local chart:
    ```bash
    helm install min ./charts/portfolios/min --namespace min
    ```

3. Verify deployments:
    ```bash
    kubectl get pods -n min
    kubectl get svc -n min
    ```

4. In a separate terminal, expose Keycloak with port-forward (keep this terminal running):
    ```bash
    kubectl port-forward -n min svc/min-keycloak 8080:8080
    ```

5. Open a browser and navigate to `localhost:8080`. Log in to Keycloak with user `admin` and password `admin` (defined in the chart values).

6. Click on "Clients" in the left pane, then create a new client. Accept defaults except:
    * `Client authentication`: set to **On**.
    * `Valid redirect URIs`: set to `http://localhost:3000/*` (or `http://localhost:3000/auth/oidc/callback`).

7. Go to the "Credentials" tab and copy the client secret. You need both client ID and client secret in the next step.

## 4. Install the app

Deploy the app model to the AKS Radius environment created above.

1. Deploy the app:
    ```bash
    cd radius
    rad deploy app.bicep \
      --group trading \
      --environment aks-trading \
      --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
      --parameters imageTag=latest \
      --parameters authUsername=admin \
      --parameters authPassword=admin \
      --parameters oidcIssuer=http://min-keycloak.min.svc.cluster.local:8080/realms/master \
      --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
      --parameters oidcBrowserAuthEndpoint=http://localhost:8080/realms/master/protocol/openid-connect/auth \
      --parameters oidcTokenEndpoint=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/token \
      --parameters oidcUserInfoEndpoint=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/userinfo \
      --parameters oidcClientId=<Keycloak client id> \
      --parameters oidcClientSecret=<Keycloak client secret> \
      --parameters workloadIdentityOidcIssuer=$AKS_OIDC_ISSUER \
      --parameters workloadIdentityServiceAccountName=default \
      --parameters aiProvider=openai \
      --parameters aiModelName=gpt-4o \
      --parameters aiApiKey=<OpenAI API key>
    ```

    > **NOTE:** The app model automatically sets `azure.workload.identity/use=true` on backend and frontend pods.

    If you want to use an Azure OpenAI deployment endpoint, set these parameters instead:

    ```bash
    --parameters aiProvider=azure-key \
    --parameters aiModelName=gpt-4 \
    --parameters aiEndpoint=https://<your-openai-account>.openai.azure.com/ \
    --parameters aiApiKey=<Azure OpenAI deployment key>
    ```

2. Expose the frontend:
    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

3. Open the app at `http://localhost:3000`.
4. Login using local account `admin/admin`, or click "Sign in with OIDC" to authenticate with Keycloak.

## 5. Clean up

1. Delete the app:
    ```bash
    rad app delete portable-apps
    ```

2. Delete the AKS resource group:
    ```bash
    az group delete --name adaptive-aks --yes --no-wait
    ```

## Troubleshoot

1. If `rad` commands are targeting a different cluster, check and switch your workspace:
    ```bash
    rad workspace show
    rad workspace switch aks-trading
    kubectl config current-context
    ```

2. If resource type registration fails, ensure you run the command from repo root or `radius` folder:
    ```bash
    cd radius
    rad resource-type create --from-file resource-types/types.yaml
    ```

3. If OIDC sign-in fails, verify the client redirect URI exactly matches:
    ```text
    http://localhost:3000/auth/oidc/callback
    ```

## Additional Topics

* [Deploy Keycloak behind an ingress](../../../docs/authentication/keycloak-ingress.md)
* [Configure Keycloak federation with Azure Entra ID](../../../docs/authentication/keycloak-entra.md)
* [Configure Keycloak federation with a local Active Directory](../../../docs/authentication/keycloak-active-directory.md)
* [Configure credential sync from local Active Directory to an Azure Entra tenant](../../../docs/authentication/microsoft-entra-connect.md)