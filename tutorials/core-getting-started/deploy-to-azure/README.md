# Deploy Adaptive App (Core) to Azure

The `core` portfolio adds an Istio service mesh with mesh-wide STRICT mTLS on
top of the `min` portfolio. On AKS, Istio is available as a managed add-on, so
the chart can either install Istio itself (the default, used for local
clusters) or defer to the AKS add-on. This tutorial uses the AKS add-on.

> **NOTE:** The AKS Istio add-on runs in the `aks-istio-system` namespace (not
> `istio-system`). Override the chart default with `--set istio.namespace=aks-istio-system`
> on every `helm install` / `helm upgrade` so the mesh-wide `PeerAuthentication`
> is applied where the add-on actually runs.

## 0. Prerequisites

* An [Azure subscription](https://portal.azure.com/)
* [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/?view=azure-cli-latest) or Cloud Shell on Azure Portal
* [Helm](https://helm.sh/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
* [rad](https://docs.radapp.io/guides/tooling/rad-cli/howto-rad-cli/)
* (optional) An [OpenAI API Key](https://platform.openai.com/api-keys) or [Azure OpenAI deployment key](https://azure.microsoft.com/en-us/products/ai-foundry/models/openai)

## 1. Define a few environment variables for consistency

```bash
export AZURE_SUBSCRIPTION=<your Azure subscription id>
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

3. Create an AKS cluster with the Istio service mesh add-on enabled:

    ```bash
    az aks create \
      --resource-group $RESOURCE_GROUP \
      --name $AKS_CLUSTER \
      --node-count 2 \
      --enable-addons monitoring \
      --generate-ssh-keys \
      --enable-oidc-issuer \
      --enable-workload-identity \
      --enable-azure-service-mesh
    ```

    > **NOTE:** If you reuse an existing AKS cluster, enable the required features:
    >
    > ```bash
    > az aks update --resource-group $RESOURCE_GROUP --name $AKS_CLUSTER \
    >   --enable-oidc-issuer --enable-workload-identity
    > az aks mesh enable --resource-group $RESOURCE_GROUP --name $AKS_CLUSTER
    > ```

4. Get AKS credentials and merge them into your `kubectl` config:

    ```bash
    az aks get-credentials --resource-group $RESOURCE_GROUP --name $AKS_CLUSTER
    ```

5. Capture the active Istio revision installed by the AKS add-on — you'll need it later to label app namespaces for sidecar injection:

    ```bash
    export ISTIO_REVISION=$(az aks show \
      --resource-group $RESOURCE_GROUP \
      --name $AKS_CLUSTER \
      --query 'serviceMeshProfile.istio.revisions[0]' -o tsv)
    echo $ISTIO_REVISION   # e.g. asm-1-24
    ```

    > **NOTE:** AKS uses revision labels (`istio.io/rev=asm-<version>`) for sidecar
    > injection, not the open-source `istio-injection=enabled` label used on
    > local clusters.

## 3. Set up Radius on AKS

1. Verify the current Kubernetes context points to AKS:

    ```bash
    kubectl config current-context
    kubectl cluster-info
    ```

2. Install the Radius control plane with workload-identity support:

    ```bash
    rad install kubernetes --set global.azureWorkloadIdentity.enabled=true
    ```

3. Create and switch to a Radius workspace bound to this AKS context:

    ```bash
    rad workspace create kubernetes $RADIUS_WORKSPACE --context $AKS_CLUSTER --force
    rad workspace switch $RADIUS_WORKSPACE
    rad workspace show
    ```

4. Capture the AKS OIDC issuer URL (used for workload-identity federation):

    ```bash
    export AKS_OIDC_ISSUER=$(az aks show \
      --resource-group $RESOURCE_GROUP \
      --name $AKS_CLUSTER \
      --query oidcIssuerProfile.issuerUrl \
      -o tsv)
    ```

5. Register Azure credentials so Radius can provision Azure resources. Use the shared `wi-helper.sh` script from the `min` tutorial folder — it creates a managed identity and federates it with the Radius service account:

    > **NOTE:** See <https://docs.radapp.io/guides/operations/providers/azure-provider/howto-azure-provider-wi/> for background.

    ```bash
    ./tutorials/min-getting-started/deploy-to-azure/wi-helper.sh \
      $AKS_CLUSTER $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER
    ```

6. Create the Radius group:

    ```bash
    rad group create $RADIUS_GROUP
    ```

7. Register custom resource types used by the sample app:

    ```bash
    cd radius
    rad resource-type create --from-file resource-types/types.yaml
    ```

8. Create the AKS Radius environment and recipe bindings:

    ```bash
    rad env create $RADIUS_WORKSPACE --group $RADIUS_GROUP
    rad deploy aks-env.bicep \
      --group $RADIUS_GROUP \
      --parameters azureSubscriptionId=$AZURE_SUBSCRIPTION \
      --parameters azureResourceGroup=$RESOURCE_GROUP

    rad environment list --group $RADIUS_GROUP
    ```

9. Register the credential created by `wi-helper.sh`:

    ```bash
    export APPLICATION_NAME=$AKS_CLUSTER-radius-app
    export APPLICATION_CLIENT_ID=$(az ad app list --display-name "$APPLICATION_NAME" --query '[].appId' -o tsv)
    export TENANT_ID=$(az account show --query tenantId -o tsv)

    rad credential register azure wi --client-id $APPLICATION_CLIENT_ID --tenant-id $TENANT_ID
    rad credential show azure
    ```

## 4. Provision workload identities for the app

Managed identities, federated credentials, and RBAC must be provisioned before
deploying the app. Use the shared `app-wi-setup.sh` script once per workload:

```bash
cd tutorials/min-getting-started/deploy-to-azure

# Create managed identity + federated credential for backend
./app-wi-setup.sh backend  $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER trading default

# Create managed identity + federated credential for frontend
./app-wi-setup.sh frontend $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER trading default
```

Capture the client IDs output by each call:

```bash
export BACKEND_CLIENT_ID=$(az identity show -g $RESOURCE_GROUP -n backend  --query clientId -o tsv)
export FRONTEND_CLIENT_ID=$(az identity show -g $RESOURCE_GROUP -n frontend --query clientId -o tsv)
```

> **NOTE:** You only need to run this step once. Re-running the deploy later does not re-provision identities.

## 5. Install Adaptive App Capability Portfolio (Core)

The `core` portfolio Helm chart bundles the `min` portfolio (Keycloak IdP) and
adds the Istio mesh layer. Because AKS owns the Istio control plane in this
tutorial, install the chart with `istio.install.enabled=false` — the chart
still applies the mesh-wide STRICT `PeerAuthentication`.

Pick **one** of the two OIDC paths below — both end with the same `core-oidc`
ConfigMap and `oidc-client` Secret in the `core` namespace, so step 6 is
identical regardless of which path you chose.

> **Why two paths?** AKS exposes an OIDC issuer
> ([docs](https://learn.microsoft.com/en-us/azure/aks/use-oidc-issuer)) for
> signing **ServiceAccount tokens** for workload identity — it is not a
> user-facing IdP. For end-user browser sign-in on AKS the natural choice is
> **Microsoft Entra ID** (Option B). Keycloak (Option A) remains useful when
> you need a self-managed IdP or want the same setup to work on non-Azure
> clusters.

### Option A — In-cluster Keycloak (default, portable)

1. Create namespace:

    ```bash
    kubectl create namespace core
    ```

2. Install the `core` portfolio with the chart's Istio install disabled (AKS add-on owns the control plane):

    ```bash
    helm dependency update ./charts/portfolios/core
    helm install core ./charts/portfolios/core --namespace core \
      --set istio.install.enabled=false \
      --set istio.namespace=aks-istio-system
    ```

3. Verify deployments. The chart still applies the mesh-wide `PeerAuthentication` in `istio-system`:

    ```bash
    kubectl get pods -n core
    kubectl get peerauthentication -n istio-system
    ```

4. In a separate terminal, expose Keycloak with port-forward (keep this terminal running):

    ```bash
    kubectl port-forward -n core svc/core-keycloak 8080:8080
    ```

5. Open a browser and navigate to `localhost:8080`. Log in to Keycloak with user `admin` and password `admin` (defined in the chart values).

6. Click on "Clients" in the left pane and create a new client. Accept defaults except:

    * `Client authentication`: set to **On**.
    * `Valid redirect URIs`: set to `http://localhost:3000/*` (or `http://localhost:3000/auth/oidc/callback`).

7. Go to the "Credentials" tab and copy the client secret. Capture the client ID and secret into shell variables:

    ```bash
    export OIDC_APP_ID=<Keycloak client id>
    export OIDC_APP_SECRET=<Keycloak client secret>
    ```

8. Render the client-secret Kubernetes Secret and re-render the OIDC ConfigMap with the captured client ID and the browser-facing endpoint (port-forward URL):

    ```bash
    kubectl -n core create secret generic oidc-client \
      --from-literal=clientSecret=$OIDC_APP_SECRET \
      --dry-run=client -o yaml | kubectl apply -f -

    helm upgrade core ./charts/portfolios/core --namespace core --reuse-values \
      --set global.oidc.clientId=$OIDC_APP_ID \
      --set global.oidc.clientSecretRef.name=oidc-client \
      --set global.oidc.browserAuthEndpoint=http://localhost:8080/realms/master/protocol/openid-connect/auth
    ```

### Option B — Microsoft Entra ID (AKS-native, no in-cluster IdP)

1. Create namespace:

    ```bash
    kubectl create namespace core
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

    > **NOTE (Microsoft / Service Tree-enforced tenants):** If `az ad app create` fails with
    > `ServiceManagementReference field is required for Create, but is missing in the request`,
    > your tenant requires every app registration to be linked to a [Service Tree](https://aka.ms/service-management-reference-error) entry.
    > Pass `--service-management-reference <service-tree-guid>` on `az ad app create`:
    >
    > ```bash
    > export SERVICE_MGMT_REF=<your-service-tree-guid>
    > export OIDC_APP_ID=$(az ad app create \
    >   --display-name portable-apps-frontend \
    >   --sign-in-audience AzureADMyOrg \
    >   --web-redirect-uris http://localhost:3000/auth/oidc/callback \
    >   --service-management-reference $SERVICE_MGMT_REF \
    >   --query appId -o tsv)
    > ```

3. Create the OIDC client-secret Kubernetes Secret:

    ```bash
    kubectl -n core create secret generic oidc-client \
      --from-literal=clientSecret=$OIDC_APP_SECRET
    ```

4. Install the `core` portfolio with Keycloak disabled, the AKS Istio add-on owning the control plane, and Entra endpoints wired in:

    ```bash
    helm dependency update ./charts/portfolios/core
    helm install core ./charts/portfolios/core --namespace core \
      --set istio.install.enabled=false \
      --set istio.namespace=aks-istio-system \
      --set global.components.keycloak.enabled=false \
      --set global.oidc.clientId=$OIDC_APP_ID \
      --set global.oidc.clientSecretRef.name=oidc-client \
      --set global.oidc.external.issuer=https://login.microsoftonline.com/$TENANT_ID/v2.0 \
      --set global.oidc.external.authEndpoint=https://login.microsoftonline.com/$TENANT_ID/oauth2/v2.0/authorize \
      --set global.oidc.external.tokenEndpoint=https://login.microsoftonline.com/$TENANT_ID/oauth2/v2.0/token \
      --set global.oidc.external.userInfoEndpoint=https://graph.microsoft.com/oidc/userinfo
    ```

    Verify no Keycloak/Postgres pods were created, the mesh-wide PeerAuthentication is in place, and the OIDC ConfigMap is in `external` mode:

    ```bash
    kubectl get pods -n core                              # observability components only
    kubectl get peerauthentication -n aks-istio-system
    kubectl get cm core-oidc -n core -o yaml              # mode: "external"
    ```

## 6. Install the app

Deploy the app model to the AKS Radius environment created above. The OIDC
values are read from the `core-oidc` ConfigMap and `oidc-client` Secret
populated by step 5 — the same commands work for both Option A and Option B.

1. Read OIDC values into shell variables:

    ```bash
    eval "$(kubectl -n core get cm core-oidc -o go-template='
    export OIDC_ISSUER={{ .data.issuer | printf "%q" }}
    export OIDC_AUTH_ENDPOINT={{ .data.authEndpoint | printf "%q" }}
    export OIDC_BROWSER_AUTH_ENDPOINT={{ .data.browserAuthEndpoint | printf "%q" }}
    export OIDC_TOKEN_ENDPOINT={{ .data.tokenEndpoint | printf "%q" }}
    export OIDC_USERINFO_ENDPOINT={{ .data.userInfoEndpoint | printf "%q" }}
    export OIDC_CLIENT_ID={{ .data.clientId | printf "%q" }}
    export OIDC_CLIENT_SECRET_NAME={{ .data.clientSecretName | printf "%q" }}
    export OIDC_CLIENT_SECRET_KEY={{ .data.clientSecretKey | printf "%q" }}
    ')"
    export OIDC_CLIENT_SECRET=$(kubectl -n core get secret "$OIDC_CLIENT_SECRET_NAME" \
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

    If you want to use an Azure OpenAI deployment endpoint, set these parameters instead:

    ```bash
    --parameters aiProvider=azure-key \
    --parameters aiModelName=gpt-4 \
    --parameters aiEndpoint=https://<your-openai-account>.openai.azure.com/ \
    --parameters aiApiKey=<Azure OpenAI deployment key>
    ```

3. Enable Istio sidecar injection on the app namespace.

    Radius creates the app namespace (`trading-<app-name>`), but it does not label it for sidecar injection. The `enableIstioInjection=true` parameter only adds the `sidecar.istio.io/inject: "true"` pod annotation, which is ignored unless the namespace is enrolled in the mesh. On AKS, the add-on uses **revision labels** rather than `istio-injection=enabled`:

    ```bash
    export APP_NAMESPACE=trading-portable-apps
    kubectl label namespace $APP_NAMESPACE istio.io/rev=$ISTIO_REVISION --overwrite
    kubectl rollout restart deployment -n $APP_NAMESPACE
    ```

    > **NOTE:** The `core` chart applies the mesh-wide `PeerAuthentication` with `mtls.mode: STRICT` regardless of who installed the control plane (chart on local k8s, AKS add-on here). It does not label app namespaces because Radius owns app-namespace creation.

4. (Optional) Verify each app pod has an `istio-proxy` sidecar. The AKS Istio add-on on recent Kubernetes versions injects `istio-proxy` as a **native sidecar** (under `.spec.initContainers` with `restartPolicy: Always`), so list both regular containers and native sidecars:

    ```bash
    kubectl get pods -n $APP_NAMESPACE -o jsonpath='{range .items[*]}{.metadata.name}{" => containers: "}{range .spec.containers[*]}{.name}{" "}{end}{"| sidecars: "}{range .spec.initContainers[?(@.restartPolicy=="Always")]}{.name}{" "}{end}{"\n"}{end}'
    ```

    You should see something like:

    ```text
    ai-agent-...  => containers: ai-agent | sidecars: istio-proxy
    backend-...   => containers: backend  | sidecars: istio-proxy
    frontend-...  => containers: frontend | sidecars: istio-proxy
    postgres-...  => containers: postgres | sidecars: istio-proxy
    ```

    A pod's `READY 2/2` column counts the app container plus the native `istio-proxy` sidecar, so `2/2` is the expected steady state once injection is in effect.

5. Expose the frontend:

    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

6. Open the app at `http://localhost:3000`.

7. Log in using local account `admin/admin`, or click "Sign in with OIDC" to authenticate with your IdP (Keycloak in Option A, Entra ID in Option B).

## 7. Clean up

1. Delete the app:

    ```bash
    rad app delete portable-apps
    ```

2. Delete the AKS resource group:

    ```bash
    az group delete --name $RESOURCE_GROUP --yes --no-wait
    ```

## Troubleshoot

1. If `rad` commands are targeting a different cluster, check and switch your workspace:

    ```bash
    rad workspace show
    rad workspace switch $RADIUS_WORKSPACE
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

4. If app pods come up without an `istio-proxy` sidecar, the namespace likely isn't labeled with the correct AKS revision. Re-check `$ISTIO_REVISION` and re-apply the label:

    ```bash
    az aks show -g $RESOURCE_GROUP -n $AKS_CLUSTER --query 'serviceMeshProfile.istio.revisions' -o tsv
    kubectl label namespace $APP_NAMESPACE istio.io/rev=$ISTIO_REVISION --overwrite
    kubectl rollout restart deployment -n $APP_NAMESPACE
    ```

5. If the mesh-wide PeerAuthentication is missing (`kubectl get peerauthentication -n aks-istio-system` is empty), the chart's post-install hook didn't run — re-run with `helm upgrade ... --reuse-values` or check `kubectl get jobs -n core` for failed hook jobs. The most common cause is forgetting `--set istio.namespace=aks-istio-system` on AKS; the job will fail with `namespaces "istio-system" not found`.

## Additional Topics

* [Istio service mesh — AKS add-on docs](https://learn.microsoft.com/en-us/azure/aks/istio-about)
* [Deploy Keycloak behind an ingress](../../../docs/authentication/keycloak-ingress.md)
* [Configure Keycloak federation with Azure Entra ID](../../../docs/authentication/keycloak-entra.md)
* [Configure Keycloak federation with a local Active Directory](../../../docs/authentication/keycloak-active-directory.md)
* [Configure credential sync from local Active Directory to an Azure Entra tenant](../../../docs/authentication/microsoft-entra-connect.md)
