# Deploy Adaptive App (Core) to Local K8s

## 0. Prerequisites

* [docker](https://docs.docker.com/)
* [Helm](https://helm.sh/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
* [rad](https://docs.radapp.io/guides/tooling/rad-cli/howto-rad-cli/)
* (optional) An [OpenAI API Key](https://platform.openai.com/api-keys) or [Azure OpenAI deployment key](https://azure.microsoft.com/en-us/products/ai-foundry/models/openai)


## 1. Prepare a local Kubernetes cluster
To demostrate local deployments, you need a local Kubernetes cluster such as [k3s](https://github.com/rancher/k3s) or [Kind](https://kind.sigs.k8s.io/), or a full-scale Kubernetes cluster. We'll use k3s in this tutorial.

1. Install k3d:

    ```bash
    curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
    ```    

2. Verify installation:

    ```bash
    k3d --version
    ```
3. Create a K3s cluster:

    ```bash
    k3d cluster create localk8s
    # Set K3D_FIX_DNS=0 helps cluster creation complete in environments where it otherwise stalls at configuring CoreDNS configmap
    ```

## 2. Set up Radius

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

## 3. Install Adaptive App Capability Portfolio (Core)

Start with the `core` portfolio Helm chart. The first bundled component is Keycloak.

1. Create namespace:

    ```bash
    kubectl create namespace core
    ```

2. Install the `core` portfolio from the local chart:

    ```bash
    helm install core ./charts/portfolios/core --namespace core --set min.nameOverride=core
    ```

3. Verify deployments:

    ```bash
    kubectl get pods -n core
    kubectl get svc -n core
    ```

    You should see services like:

    ```bash
    NAME                      TYPE        
    core-keycloak              ClusterIP   
    core-keycloak-discovery    ClusterIP
    core-keycloak-postgresql   ClusterIP 
    istiod                     ClusterIP
    ```

4. In a separate Terminal, expose Keycloak with port-forward (keep this terminal running):

    ```bash
    kubectl port-forward -n core svc/core-keycloak 8080:8080
    ```

5. Open a browser and navigate to `localhost:8080`. Log in to KeyCloak portal with user `admin` and password `admin` (which are defined in the `values.yaml` for the Helm chart).

6. Click on "Clients" in the left pane, and click on the "Create Client" button to create a new client. Set up a name for the client and accept all default values across screens except for:

    * `Cleint authentication`: set to **On**.
    * `Valid redirect URIs`: set to `http://localhost:3000/*` (or more specifically `http://localhost:3000/auth/oidc/callback`). 
    
    Click "Save" to save the client definition.

7. Go to "Credentials" tab and copy the client secret. You'll need both client id and secret for the next step.

## 4. Install the app

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
    --parameters oidcIssuer=http://min-keycloak.min.svc.cluster.local:8080/realms/master \
    --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
    --parameters oidcBrowserAuthEndpoint=http://localhost:8080/realms/master/protocol/openid-connect/auth \
    --parameters oidcTokenEndpoint=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/token \
    --parameters oidcUserInfoEndpoint=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/userinfo \
    --parameters oidcClientId=<Keycloak client id> \
    --parameters oidcClientSecret=<Keycloak client secret> \
    --parameters aiProvider=openai \
    --parameters aiModelName=gpt-4o \
    --parameters <OpenAI / Azure OpenAI Service API key>

    ```

    > **NOTE:** The Keycloak service is `ClusterIP`, which is ideal for in-cluster calls from the frontend pod. This setup uses two different URLs: Browser redirects to `http://localhost:8080` (via `oidcBrowserAuthEndpoint`); Token and userinfo requests go to the in-cluster `min-keycloak.min.svc.cluster.local` (via explicit `oidcTokenEndpoint` and `oidcUserInfoEndpoint`). This causes an issuer mismatch: Keycloak issues a token with iss claim set to `http://localhost:8080/realms/master` (the URL used during authentication), but the frontend validates the token against `oidcIssuer=http://min-keycloak.min.svc.cluster.local:8080/realms/master` by default. Use `oidcIssuerOverride=http://localhost:8080/realms/master` to tell the frontend which issuer to expect. In production, Keycloak is typically deployed behind an ingress with a single DNS name used everywhere, avoiding this split-URL issue. See [keycloak-ingress.md](../../../docs/authentication/keycloak-ingress.md) for setup details.

    If you want to use an Azure OpenAI deployment endpoint, you need to set these parameters accordingly:

    ```bash
    --parameters aiProvider=azure-key \
    --parameters aiModelName=gpt-4 \
    --parameters aiEndpoint=https://antho-openai.openai.azure.com/ \
    --parameters aiApiKey=<Azure OpenAI service deployment key>
    ```
2. (Optional) Observe mTLS

    ```bash
    export APP_NAMESPACE=trading-portable-apps
    kubectl get pods -n $APP_NAMESPACE -o jsonpath='{range .items[*]}{.metadata.name}{" => "}{range .spec.containers[*]}{.name}{" "}{end}{"\n"}{end}'
    ```
    
    You should see something like:

    ```bash
    ai-agent-... => ai-agent istio-proxy
    backend-... => backend istio-proxy
    frontend-... => frontend istio-proxy
    mosquitto-... => mosquitto istio-proxy
    otel-collector-... => otel-collector istio-proxy
    postgres-... => postgres istio-proxy
    prometheus-... => prometheus istio-proxy
    zipkin-... => zipkin istio-proxy
    ```

    >**NOTE:** The `core` Helm chart handles all three steps automatically: 1. The pre-install hook installs Istio (`istio-base` + `istiod`) into `istio-system`. 2. The `namespace-enrollment` template labels the app namespace with `istio-injection=enabled`. 3. The post-install hook applies a `PeerAuthentication` with `mtls.mode: STRICT`.

This is intentionally independent of the app model in `radius/app.bicep`. The app stays portable; the environment decides whether service-to-service traffic is meshed.

3. Expose the frontend:

    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

4. Open the app at `http://localhost:3000`.
5. Login using local account admin/admin, or click on "Sign in with OIDC" button to use KeyCloak to login with federated credential.

## 5. Clean up

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
    k3d cluster create localk8s --k3s-arg "--resolv-conf=/etc/reslov.conf@server:0"
    ```
2. If you have podman also enabled, it may interfer with K3s and Docker operations, depending on how your system is configured. Make sure podman is stopped:
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
* [Configure KeyCloak federation with Azure Entra ID](../../../docs/authentication/keycloak-entra.md)
* [Configure Keycloak federation with a local Active Directory](../../../docs/authentication/keycloak-active-directory.md)
* [Configure credential sync from local Active Directory to an Azure Entra tenant](../../../docs/authentication/microsoft-entra-connect.md)