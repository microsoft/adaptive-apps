# Deploy Adaptive App (Min) to Local K8s

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

## 3. Install Adaptive App Capability Portfolio (Min)

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

5. Open a browser and navigate to `localhost:8080`. Log in to KeyCloak portal with user `admin` and password `admin` (which are defined in the `values.yaml` for the Helm chart).

6. Click on "Clients" in the left pane, and click on the "Create Client" button to create a new client. Set up a name for the client and accept all default values across screens except for:

    * `Cleint authentication`: set to **On**.
    * `Valid redirect URIs`: set to `http://localhost:3000/*` (or more specifically `http://localhost:3000/auth/oidc/callback`). 
    
    Click "Save" to save the client definition.

7. Go to "Credentials" tab and copy the client secret. Capture both values into shell variables:

    ```bash
    export OIDC_APP_ID=<Keycloak client id>
    export OIDC_APP_SECRET=<Keycloak client secret>
    ```

8. Render the client-secret Secret and re-render the chart's OIDC ConfigMap so it carries the client ID, the secret reference, and the port-forward browser endpoint:

    ```bash
    kubectl -n min create secret generic oidc-client \
      --from-literal=clientSecret=$OIDC_APP_SECRET \
      --dry-run=client -o yaml | kubectl apply -f -

    helm upgrade min ./charts/portfolios/min --namespace min --reuse-values \
      --set global.oidc.clientId=$OIDC_APP_ID \
      --set global.oidc.clientSecretRef.name=oidc-client \
      --set global.oidc.browserAuthEndpoint=http://localhost:8080/realms/master/protocol/openid-connect/auth
    ```

9. Hydrate OIDC env vars from the `min-oidc` ConfigMap (consumed by the next section's `rad deploy`):

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
    --parameters <OpenAI / Azure OpenAI Service API key>

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
4. Login using local account admin/admin, or click on "Sign in with OIDC" button to use KeyCloak to login with federated credential.

This is intentionally independent of the app model in `radius/app.bicep`. The app stays portable; the environment decides whether service-to-service traffic is meshed.

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