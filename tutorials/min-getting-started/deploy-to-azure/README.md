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

## 4. Provision workload identities for the app

Managed identities, federated credentials, and RBAC must be provisioned before
deploying the app. Use `app-wi-setup.sh` once per workload identity:

```bash
cd tutorials/min-getting-started/deploy-to-azure

# Create managed identity + federated credential for backend
./app-wi-setup.sh backend $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER trading default

# Create managed identity + federated credential for frontend
./app-wi-setup.sh frontend $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER trading default
```

Capture the client IDs output by each call:
```bash
export BACKEND_CLIENT_ID=$(az identity show -g $RESOURCE_GROUP -n backend --query clientId -o tsv)
export FRONTEND_CLIENT_ID=$(az identity show -g $RESOURCE_GROUP -n frontend --query clientId -o tsv)
```

> **NOTE:** You only need to run this step once. Re-running the deploy later does not re-provision identities.

## 5. Install Adaptive App Capability Portfolio

The `min` portfolio Helm chart provides the OIDC identity provider for user
sign-in. Pick **one** of the two paths below — they both end with the same
`min-oidc` ConfigMap and `oidc-client` Secret in the `min` namespace, so
step 6 (deploying the app) is identical regardless of which path you chose.

> **Why two paths?** AKS exposes an OIDC issuer
> ([docs](https://learn.microsoft.com/en-us/azure/aks/use-oidc-issuer)), but
> that issuer signs **ServiceAccount tokens for workload identity** — it is
> not a user-facing IdP. For end-user browser sign-in on AKS the natural
> choice is **Microsoft Entra ID** (Option B). Keycloak (Option A) remains
> useful when you need a self-managed IdP or want the same setup to work
> on non-Azure clusters.

### Option A — In-cluster Keycloak (default, portable)

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

7. Go to the "Credentials" tab and copy the client secret. Capture the client ID and secret into shell variables:
    ```bash
    export OIDC_APP_ID=<Keycloak client id>
    export OIDC_APP_SECRET=<Keycloak client secret>
    ```

8. Render the client-secret Kubernetes Secret and re-render the OIDC ConfigMap with the captured client ID and the browser-facing endpoint (port-forward URL):
    ```bash
    kubectl -n min create secret generic oidc-client \
      --from-literal=clientSecret=$OIDC_APP_SECRET \
      --dry-run=client -o yaml | kubectl apply -f -

    helm upgrade min ./charts/portfolios/min --namespace min --reuse-values \
      --set oidc.clientId=$OIDC_APP_ID \
      --set oidc.clientSecretRef.name=oidc-client \
      --set oidc.browserAuthEndpoint=http://localhost:8080/realms/master/protocol/openid-connect/auth
    ```

### Option B — Microsoft Entra ID (AKS-native, no in-cluster IdP)

1. Create namespace:
    ```bash
    kubectl create namespace min
    ```

2. Register an Entra application for the frontend's browser sign-in and capture the client ID / secret:
    ```bash
    export TENANT_ID=$(az account show --query tenantId -o tsv)
    export OIDC_APP_ID=$(az ad app create \
      --display-name portable-apps-frontend \
      --sign-in-audience AzureADMyOrg \
      --web-redirect-uris http://localhost:3000/auth/oidc/callback \
      --query appId -o tsv)
    export OIDC_APP_SECRET=$(az ad app credential reset \
      --id $OIDC_APP_ID --append \
      --query password -o tsv)
    ```

    > **NOTE:** `AzureADMyOrg` restricts sign-in to your tenant. Use `AzureADMultipleOrgs` for multi-tenant. Add additional redirect URIs (`az ad app update --web-redirect-uris ...`) when you front the app with a public ingress.

3. Create the OIDC client-secret Kubernetes Secret:
    ```bash
    kubectl -n min create secret generic oidc-client \
      --from-literal=clientSecret=$OIDC_APP_SECRET
    ```

4. Install the `min` portfolio with Keycloak disabled and Entra endpoints wired in:
    ```bash
    helm install min ./charts/portfolios/min --namespace min \
      --set components.keycloak.enabled=false \
      --set oidc.clientId=$OIDC_APP_ID \
      --set oidc.clientSecretRef.name=oidc-client \
      --set oidc.external.issuer=https://login.microsoftonline.com/$TENANT_ID/v2.0 \
      --set oidc.external.authEndpoint=https://login.microsoftonline.com/$TENANT_ID/oauth2/v2.0/authorize \
      --set oidc.external.tokenEndpoint=https://login.microsoftonline.com/$TENANT_ID/oauth2/v2.0/token \
      --set oidc.external.userInfoEndpoint=https://graph.microsoft.com/oidc/userinfo
    ```

    Verify no Keycloak/Postgres pods were created — only the ConfigMap:
    ```bash
    kubectl get all -n min                     # should be empty
    kubectl get cm min-oidc -n min -o yaml     # mode: "external"
    ```

## 6. Install the app

Deploy the app model to the AKS Radius environment created above. The OIDC
values are read from the `min-oidc` ConfigMap and `oidc-client` Secret
populated by step 5 — the same commands work for both Option A and Option B.

1. Read OIDC values into shell variables:
    ```bash
    eval "$(kubectl -n min get cm min-oidc -o go-template='
    export OIDC_ISSUER={{ .data.issuer | printf "%q" }}
    export OIDC_AUTH_ENDPOINT={{ .data.authEndpoint | printf "%q" }}
    export OIDC_BROWSER_AUTH_ENDPOINT={{ .data.browserAuthEndpoint | printf "%q" }}
    export OIDC_TOKEN_ENDPOINT={{ .data.tokenEndpoint | printf "%q" }}
    export OIDC_USERINFO_ENDPOINT={{ .data.userInfoEndpoint | printf "%q" }}
    export OIDC_CLIENT_ID={{ .data.clientId | printf "%q" }}
    export OIDC_CLIENT_SECRET_NAME={{ .data.clientSecretName | printf "%q" }}
    export OIDC_CLIENT_SECRET_KEY={{ .data.clientSecretKey | printf "%q" }}
    ')"
    export OIDC_CLIENT_SECRET=$(kubectl -n min get secret "$OIDC_CLIENT_SECRET_NAME" \
      -o jsonpath="{.data.${OIDC_CLIENT_SECRET_KEY}}" | base64 -d)
    ```

    > **NOTE:** If you used Option A (Keycloak via port-forward), the in-cluster
    > `OIDC_ISSUER` URL uses the Keycloak Service DNS, but Keycloak issues
    > tokens with an `iss` claim equal to the URL the browser used during
    > authentication (`http://localhost:8080/realms/master`). Pass that as
    > `oidcIssuerOverride` to `rad deploy` so the frontend validates the
    > token against the right issuer. Option B has no such mismatch.

2. Deploy the app:
    ```bash
    cd radius
    rad deploy app.bicep \
      --group trading \
      --environment aks-trading \
      --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
      --parameters imageTag=latest \
      --parameters authUsername=admin \
      --parameters authPassword=admin \
      --parameters otelCollectorEndpoint=http://otel-collector.core:4318 \
      --parameters oidcIssuer=$OIDC_ISSUER \
      --parameters oidcAuthEndpoint=$OIDC_AUTH_ENDPOINT \
      --parameters oidcBrowserAuthEndpoint=$OIDC_BROWSER_AUTH_ENDPOINT \
      --parameters oidcTokenEndpoint=$OIDC_TOKEN_ENDPOINT \
      --parameters oidcUserInfoEndpoint=$OIDC_USERINFO_ENDPOINT \
      --parameters oidcClientId=$OIDC_CLIENT_ID \
      --parameters oidcClientSecret=$OIDC_CLIENT_SECRET \
      --parameters workloadIdentityServiceAccountName=default \
      --parameters backendClientId=$BACKEND_CLIENT_ID \
      --parameters frontendClientId=$FRONTEND_CLIENT_ID \
      --parameters aiProvider=openai \
      --parameters aiModelName=gpt-4o \
      --parameters aiApiKey=<OpenAI API key>
    ```

    For **Option A only**, also add the issuer override so the frontend
    accepts the browser-issued token:

    ```bash
      --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
    ```

    > **NOTE:** The app model automatically sets `azure.workload.identity/use=true` on backend and frontend pods.

    If you want to use an Azure OpenAI deployment endpoint, set these parameters instead:

    ```bash
    --parameters aiProvider=azure-key \
    --parameters aiModelName=gpt-4 \
    --parameters aiEndpoint=https://<your-openai-account>.openai.azure.com/ \
    --parameters aiApiKey=<Azure OpenAI deployment key>
    ```

3. Expose the frontend:
    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

4. Open the app at `http://localhost:3000`.
5. Login using local account `admin/admin`, or click "Sign in with OIDC" to authenticate with your IdP (Keycloak in Option A, Entra ID in Option B).

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