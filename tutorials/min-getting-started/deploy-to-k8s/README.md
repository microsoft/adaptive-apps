# Deploy Adaptive App (Min) to Local K8s

## 0. Prerequisites

* [docker](https://docs.docker.com/)
* [Helm](https://helm.sh/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
* [rad](https://docs.radapp.io/guides/tooling/rad-cli/howto-rad-cli/)
* (optional) An [OpenAI API Key](https://platform.openai.com/api-keys) or [Azure OpenAI deployment key](https://azure.microsoft.com/en-us/products/ai-foundry/models/openai)

## OPTION 1: Use Adaptive App Tools

The Adaptive App CLI provides a streamlined experience of configuring everything you need to get ready for a Radius application deployment. Use this tool if you want to quickly set up a test/demo environment. Or, you can follow the manual steps in OPTION 2 below.

### 1. Set up the infrastructure

1. Setup the Adaptive App CLI.

    Follow instructions [here](../../common/prepare-cli.md) to set up Adaptive App CLI.

2.  Bootstrap the infrastructure. This sets up a local K3s cluster, installs Radius, registers Radius resource types and prepares Radius group and environment. It also automates creation of Keycloak client secret and enables port forwarding on the Keycloak service.

    ```bash
    ada bootstrap --portfolio min --platform k3s --release min --namespace min --with-radius --keep-port-forward
    ```

    The command generates a number of `export` commands. Copy those commands for the next step.
    
3. In another terminal window, execute the above `export` commands to set the environment variables.

    ```bash
    export OIDC_CLIENT_ID=portable-apps
    export OIDC_CLIENT_SECRET=<OIDC client secret>
    export OIDC_ISSUER=http://min-keycloak.min.svc.cluster.local:8080/realms/master
    export OIDC_AUTH_ENDPOINT=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/auth
    export OIDC_TOKEN_ENDPOINT=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/token
    export OIDC_USERINFO_ENDPOINT=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/userinfo
    export OIDC_BROWSER_AUTH_ENDPOINT=http://localhost:8080/realms/master/protocol/openid-connect/auth
    ```

### 2. Deploy and test the sample app

1. Deploy the Radius app:

    ```bash
    # under the radius folder
    rad deploy app.bicep \
    --group adaptive \
    --environment trading \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=admin \
    --parameters otelCollectorEndpoint=http://otel-collector.core:4318 \
    --parameters oidcIssuer=$OIDC_ISSUER \
    --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
    --parameters oidcAuthEndpoint=$OIDC_AUTH_ENDPOINT \
    --parameters oidcBrowserAuthEndpoint=$OIDC_BROWSER_AUTH_ENDPOINT \
    --parameters oidcTokenEndpoint=$OIDC_TOKEN_ENDPOINT \
    --parameters oidcUserInfoEndpoint=$OIDC_USERINFO_ENDPOINT \
    --parameters oidcClientId=$OIDC_CLIENT_ID \
    --parameters oidcClientSecret=$OIDC_CLIENT_SECRET \
    --parameters aiProvider=openai \
    --parameters aiModelName=gpt-4o \
    --parameters aiApiKey=<OpenAI API Key>
    ```

2. Expose the frontend:

    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

3. Open the app at `http://localhost:3000`.

4. Login using local account admin/admin, or click on "Sign in with OIDC" button to use Keycloak to login with federated credential.

### 3. Clean up

1. Delete the app:

    ```
    rad app delete portable-apps
    ```

2. Delete the K3s cluster:

    ```
    k3d cluster delete localk8s
    ```

## OPTION 2: Manual Setup

### 1. Prepare a local Kubernetes cluster
To demonstrate local deployments, you need a local Kubernetes cluster such as [k3s](https://github.com/rancher/k3s) or [Kind](https://kind.sigs.k8s.io/), or a full-scale Kubernetes cluster. We'll use k3s in this tutorial.

Follow instructions [here](../../common/prepare-k3s.md) to provision a K3s cluster.

### 2. Set up Radius

Set up Radius on the local cluster and register the custom resource types used by the app model.

1. Select the local Kubernetes context:

    ```bash
    kubectl config use-context k3d-localk8s
    ```

2. Install Radius control plane:

    ```bash
    rad install kubernetes --set rp.publicEndpointOverride=localhost:8081
    ```

3. Create and switch to a Radius workspace bound to this cluster:

    ```bash
    rad workspace create kubernetes trading --context k3d-localk8s --force
    rad workspace switch trading
    rad workspace show
    ```

4. Create the Radius group:

    ```bash
    rad group create trading
    ```

5. Register custom resource types used by the sample app:

    ```bash
    cd radius
    rad resource-type create --from-file resource-types/types.yaml
    ```

6. Create the local Radius environment and recipe bindings:

    ```bash
    rad environment create trading
    rad deploy local-env.bicep --group trading
    rad environment list --group trading
    ```

### 3. Install Adaptive App Capability Portfolio (Min)

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

    You should see services like:

    ```bash
    NAME                      TYPE        
    min-keycloak              ClusterIP   
    min-keycloak-discovery    ClusterIP
    min-keycloak-postgresql   ClusterIP 
    ```

4. In a separate Terminal, expose Keycloak with port-forward (keep this terminal running):

    ```bash
    kubectl port-forward -n min svc/min-keycloak 8080:8080
    ```

5. Open a browser and navigate to `localhost:8080`. Log in to Keycloak portal with user `admin` and password `admin` (which are defined in the `values.yaml` for the Helm chart).

6. Click on "Clients" in the left pane, and click on the "Create Client" button to create a new client. Set up a name for the client and accept all default values across screens except for:

    * `Client authentication`: set to **On**.
    * `Valid redirect URIs`: set to `http://localhost:3000/*` (or more specifically `http://localhost:3000/auth/oidc/callback`). 
    
    Click "Save" to save the client definition.

7. Go to "Credentials" tab and copy the client secret. Capture both values into shell variables:

    ```bash
    export OIDC_APP_ID=<Keycloak client id>
    export OIDC_APP_SECRET=<Keycloak client secret>
    ```

8. Build the OIDC env vars for the next section's `rad deploy`. The chart's `min-oidc` ConfigMap already exposes the in-cluster endpoint URLs; the client ID/secret come straight from Keycloak, and the browser endpoint is the port-forward URL:

    ```bash
    eval "$(kubectl -n min get cm min-oidc -o go-template='
    export OIDC_ISSUER={{ .data.issuer | printf "%q" }}
    export OIDC_AUTH_ENDPOINT={{ .data.authEndpoint | printf "%q" }}
    export OIDC_TOKEN_ENDPOINT={{ .data.tokenEndpoint | printf "%q" }}
    export OIDC_USERINFO_ENDPOINT={{ .data.userInfoEndpoint | printf "%q" }}
    ')"
    export OIDC_CLIENT_ID=$OIDC_APP_ID
    export OIDC_CLIENT_SECRET=$OIDC_APP_SECRET
    export OIDC_BROWSER_AUTH_ENDPOINT=http://localhost:8080/realms/master/protocol/openid-connect/auth
    ```

### 4. Install the app

Deploy the app model to the Radius environment created above.

1. Deploy the app:

    ```bash
    cd radius
    rad deploy app.bicep \
    --group trading \
    --environment trading \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=admin \
    --parameters otelCollectorEndpoint=http://otel-collector.core:4318 \
    --parameters oidcIssuer=$OIDC_ISSUER \
    --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
    --parameters oidcAuthEndpoint=$OIDC_AUTH_ENDPOINT \
    --parameters oidcBrowserAuthEndpoint=$OIDC_BROWSER_AUTH_ENDPOINT \
    --parameters oidcTokenEndpoint=$OIDC_TOKEN_ENDPOINT \
    --parameters oidcUserInfoEndpoint=$OIDC_USERINFO_ENDPOINT \
    --parameters oidcClientId=$OIDC_CLIENT_ID \
    --parameters oidcClientSecret=$OIDC_CLIENT_SECRET \
    --parameters aiProvider=openai \
    --parameters aiModelName=gpt-4o \
    --parameters aiApiKey=<OpenAI / Azure OpenAI Service API key>

    ```

    > **NOTE:** The OIDC values above are read from the chart-rendered `min-oidc` ConfigMap. Users reach Keycloak via `kubectl port-forward` on `localhost:8080`, while the frontend pod calls it via the in-cluster Service DNS, so Keycloak issues tokens with `iss=http://localhost:8080/realms/master` (the URL used during browser auth). `oidcIssuerOverride=http://localhost:8080/realms/master` makes the frontend accept tokens validated against the localhost issuer while still hitting the token/userinfo endpoints in-cluster. In production, expose Keycloak behind an ingress with a single DNS name to avoid this split-URL setup — see [keycloak-ingress.md](../../../docs/authentication/keycloak-ingress.md).

    If you want to use an Azure OpenAI deployment endpoint, you need to set these parameters accordingly:

    ```bash
    --parameters aiProvider=azure-key \
    --parameters aiModelName=gpt-4 \
    --parameters aiEndpoint=https://antho-openai.openai.azure.com/ \
    --parameters aiApiKey=<Azure OpenAI service deployment key>
    ```

2. Expose the frontend:

    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

3. Open the app at `http://localhost:3000`.
4. Login using local account admin/admin, or click on "Sign in with OIDC" button to use Keycloak to login with federated credential.


### 5. Clean up

1. Delete the app:

    ```
    rad app delete portable-apps
    ```

2. Delete the K3s cluster:

    ```
    k3d cluster delete localk8s
    ```

## Troubleshoot

1. Sometimes K3s DNS resolution is not initialized correctly when launched in WSL, leading DNS resolution failures in pods. Try to recreate the cluster using resolv file on the host:

    ```bash
    k3d cluster delete localk8s
    k3d cluster create localk8s --k3s-arg "--resolv-conf=/etc/resolv.conf@server:0"
    ```
2. If you have podman also enabled, it may interfere with K3s and Docker operations, depending on how your system is configured. Make sure podman is stopped:
    ```bash
    systemctl --user stop podman.socket
    systemctl --user stop podman.service 
    ```
    And check if your `DOCKER_HOST` is pointing to podman. If so, unset it:
    ```bash
    unset DOCKER_HOST
    ```
3. To observe Keycloak client authentication events, in Keycloak portal, go to **Realm settings** -> **Events** -> **User event settings** and turn **Save events** to **On**.

4. If you need to change/customize the application containers like the frontend container or the backend container, you can build your own images and then override the `imageRegistry` parameter and `imageTag` parameter to point to your own images. If the images are not pushed to a public repository, you can manually import them to K3s:
    ```bash
    k3d image import <your image tag> -c localk8s
    ```

## Additional Topics

* [Deploy Keycloak behind an ingress](../../../docs/authentication/keycloak-ingress.md)
* [Configure Keycloak federation with Azure Entra ID](../../../docs/authentication/keycloak-entra.md)
* [Configure Keycloak federation with a local Active Directory](../../../docs/authentication/keycloak-active-directory.md)
* [Configure credential sync from local Active Directory to an Azure Entra tenant](../../../docs/authentication/microsoft-entra-connect.md)