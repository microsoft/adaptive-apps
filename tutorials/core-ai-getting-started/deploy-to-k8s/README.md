# Deploy Adaptive App (Core-AI) to Local K8s

## 0. Prerequisites

* NVIDIA GPU with >= 4G memory, preferrably >= 16G 
* Latest NVIDIA driver installed (with WSL support if using WSL)
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
3. Create a K3s cluster using a custom Docker image and a custom volume folder:

    ```bash
    mkdir -p ~/k3d/localk8s-storage

    k3d cluster create localk8s \
    --image hbai/cuda:0.1 \
    --gpus all \
    --k3s-arg "--disable=traefik@server:0" \
    --volume "$HOME/k3d/localk8s-storage:/var/lib/rancher/k3s@server:0" \
    --volume "/usr/lib/wsl:/usr/lib/wsl@server:0" \
    --volume "/dev/dxg:/dev/dxg@server:0"
    ```

    > **NOTE:** To build hbai/cuda:0.1 package, use the `Dockerfile.nvidia` file under the `tutorials/min-ai-getting-started/deploy-to-k8s` folder: docker build -t <tag> -f Dockerfile.nvidia .

4. Install kyverno. For Kaito to work with K3s, we need a cluster policy to patch statefulset with nvidia runtimeClassName:

    ```bash
    helm repo add kyverno https://kyverno.github.io/kyverno/
    helm repo update

    helm upgrade --install kyverno kyverno/kyverno \
    -n kyverno \
    --create-namespace
    ```

5. Apply the GPU bootstrap artifact:

    ```bash
    kubectl apply -f tutorials/min-ai-getting-started/deploy-to-k8s/gpu_bootstrap.yaml
    ```

6. Run `validate_gpu.sh` to validate the node has allocatable GPU:

    ```bash
    tutorials/min-ai-getting-started/deploy-to-k8s/validate_gpu.sh
    # then Press Ctrl+C to exit
    ````
    You should see something like:
    ```bash
    Fri May 15 09:06:53 2026
    +-----------------------------------------------------------------------------------------+
    | NVIDIA-SMI 590.48.01              Driver Version: 591.55         CUDA Version: 13.1     |
    +-----------------------------------------+------------------------+----------------------+
    | GPU  Name                 Persistence-M | Bus-Id          Disp.A | Volatile Uncorr. ECC |
    | Fan  Temp   Perf          Pwr:Usage/Cap |           Memory-Usage | GPU-Util  Compute M. |
    |                                         |                        |               MIG M. |
    |=========================================+========================+======================|
    |   0  NVIDIA RTX A2000 Laptop GPU    On  |   00000000:F3:00.0 Off |                  N/A |
    | N/A   60C    P5              7W /   35W |    1335MiB /   4096MiB |      9%      Default |
    |                                         |                        |                  N/A |
    +-----------------------------------------+------------------------+----------------------+

    +-----------------------------------------------------------------------------------------+
    | Processes:                                                                              |
    |  GPU   GI   CI              PID   Type   Process name                        GPU Memory |
    |        ID   ID                                                               Usage      |
    |=========================================================================================|
    |  No running processes found                                                             |
    +-----------------------------------------------------------------------------------------+
    ```

7. Kaito's node estimator reads the `nvidia.com/gpu.memory` label to calculate how many nodes are needed. If you were on WSL, WSL2 GPU Feature Discovery may not auto-populate all labels. Ensure the node has at least:
    ```bash
    # get NODE_NAME via kubectl get nodes
    # <MiB> should equal to GPU memory reported in step 4. See also known issue 2 below.
    export NODE_NAME=k3d-localk8s-server-0 
    kubectl label node $NODE_NAME nvidia.com/gpu=true          # Kaito labelSelector
    kubectl label node $NODE_NAME nvidia.com/gpu.product=Persistence-M # Should match with your GPUs
    kubectl label node $NODE_NAME nvidia.com/gpu.present=true
    kubectl label node $NODE_NAME nvidia.com/gpu.count=1
    kubectl label node $NODE_NAME nvidia.com/gpu.memory=<MiB>  # Used by the estimator. See troubleshoot guide #5 below.
    ```

    Known issues (Kaito v0.9.0)

    |Issue | Workaround |
    |--------|--------|
    | Webhook panic (MustParse("")) when applying a generic-model Workspace in BYO mode | Delete the validating webhook before applying: `kubectl delete validatingwebhookconfiguration validation.workspace.kaito.sh` |
    | Node estimator `ignores max-model-len` from ConfigMap and over-estimates `targetNodeCount` |	Inflate the `nvidia.com/gpu.memory` node label to satisfy the estimator |

## 2. Set up Kaito

1. Install Kaito
    ```
    helm repo add kaito https://kaito-project.github.io/kaito/charts/kaito
    helm repo update
    helm upgrade --install kaito-workspace kaito/workspace \
    --create-namespace \
    --version 0.10.0 \
    --namespace kaito-workspace \
    --set featureGates.disableNodeAutoProvisioning=true \
    --set nvidiaDevicePlugin.enabled=false \
    --set localCSIDriver.useLocalCSIDriver=false
    ```
    > **NOTE:** Minimum required Kaito version is 0.9.0. `nvidiaDevicePlugin.enabled=false` avoids conflict with an existing device-plugin DaemonSet. `disableNodeAutoProvisioning=true` is required for BYO GPU mode (note: the flag changed to lowercase `d` in v0.9.0).

## 3. Set up Radius

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

## 3. Install Adaptive App Capability Portfolio (Core-AI)

Start with the `core-ai` portfolio Helm chart. 

1. Create namespace:

    ```bash
    kubectl create namespace core-ai
    ```

2. Install the `core-ai` portfolio from the local chart:

    ```bash
    helm install core-ai ./charts/portfolios/core-ai --namespace core-ai
    ```

3. Verify deployments:

    ```bash
    kubectl get pods -n core-ai
    kubectl get svc -n core-ai
    ```

    You should see services like:

    ```bash
    NAME                          TYPE        
    core-ai-keycloak              ClusterIP   
    core-ai-keycloak-discovery    ClusterIP
    core-ai-keycloak-postgresql   ClusterIP 
    istiod                        ClusterIP
    ```

4. In a separate Terminal, expose Keycloak with port-forward (keep this terminal running):

    ```bash
    kubectl port-forward -n core-ai svc/core-ai-keycloak 8080:8080
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
    kubectl -n core-ai create secret generic oidc-client \
      --from-literal=clientSecret=$OIDC_APP_SECRET \
      --dry-run=client -o yaml | kubectl apply -f -

    helm upgrade core-ai ./charts/portfolios/core-ai --namespace core-ai --reuse-values \
      --set global.oidc.clientId=$OIDC_APP_ID \
      --set global.oidc.clientSecretRef.name=oidc-client \
      --set global.oidc.browserAuthEndpoint=http://localhost:8080/realms/master/protocol/openid-connect/auth
    ```

9. Hydrate OIDC env vars from the `core-ai-oidc` ConfigMap (consumed by the next section's `rad deploy`):

    ```bash
    eval "$(kubectl -n core-ai get cm core-ai-oidc -o go-template='
    export OIDC_ISSUER={{ .data.issuer | printf "%q" }}
    export OIDC_AUTH_ENDPOINT={{ .data.authEndpoint | printf "%q" }}
    export OIDC_BROWSER_AUTH_ENDPOINT={{ .data.browserAuthEndpoint | printf "%q" }}
    export OIDC_TOKEN_ENDPOINT={{ .data.tokenEndpoint | printf "%q" }}
    export OIDC_USERINFO_ENDPOINT={{ .data.userInfoEndpoint | printf "%q" }}
    export OIDC_CLIENT_ID={{ .data.clientId | printf "%q" }}
    export OIDC_CLIENT_SECRET_NAME={{ .data.clientSecretName | printf "%q" }}
    export OIDC_CLIENT_SECRET_KEY={{ .data.clientSecretKey | printf "%q" }}
    ')"
    export OIDC_CLIENT_SECRET=$(kubectl -n core-ai get secret "$OIDC_CLIENT_SECRET_NAME" \
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
    --parameters aiProvider=local \
    --parameters aiModel=Qwen/Qwen3-0.6B

    ```

    > **NOTE:** The OIDC values above are read from the chart-rendered `core-ai-oidc` ConfigMap. Users reach Keycloak via `kubectl port-forward` on `localhost:8080`, while the frontend pod calls it via the in-cluster Service DNS, so Keycloak issues tokens with `iss=http://localhost:8080/realms/master` (the URL used during browser auth). `oidcIssuerOverride=http://localhost:8080/realms/master` makes the frontend accept tokens validated against the localhost issuer while still hitting the token/userinfo endpoints in-cluster. In production, expose Keycloak behind an ingress with a single DNS name to avoid this split-URL setup — see [keycloak-ingress.md](../../../docs/authentication/keycloak-ingress.md).

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
    postgres-... => postgres istio-proxy
    ```

    >**NOTE:** The `core` Helm chart handles mTLS automatically: 1. The pre-install hook installs Istio (`istio-base` + `istiod`) into `istio-system`. 2. The `namespace-enrollment` template labels the app namespace with `istio-injection=enabled`. 3. The post-install hook applies a `PeerAuthentication` with `mtls.mode: STRICT`. The chart also deploys observability components (OpenTelemetry collector, Prometheus, and Zipkin) in the `core` namespace, and the app automatically sends telemetry to the collector.

This separation allows apps to remain portable; the environment (Helm chart) decides whether observability and mTLS are available.

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