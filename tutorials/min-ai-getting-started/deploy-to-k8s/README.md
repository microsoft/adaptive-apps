# Deploy Adaptive App (Min-AI) to Local K8s

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
3. Create a K3s cluster:

    ```bash
    k3d cluster create localk8s
    # Set K3D_FIX_DNS=0 helps cluster creation complete in environments where it otherwise stalls at configuring CoreDNS configmap
    ```
4. Verify GPU and NVIDIA driver is in place:

    ```bash
    nvidia-smi
    ```
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
5. Install NVIDIA Container Toolkit:

    ```bash
    curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey \
    | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg

    curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list \
    | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \
    | sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list

    sudo apt-get update
    sudo apt-get install -y nvidia-container-toolkit
    ```
6. Configure Docker to use NVIDIA runtime
    ```bash
    sudo nvidia-ctk runtime configure --runtime=docker # this modifies /etc/docker/daemon.json automatically
    sudo systemctl restart docker
    ```
7. Test CUDA container:
    ```bash
    docker run --rm --gpus all nvidia/cuda:12.3.2-base-ubuntu22.04 nvidia-smi
    ```
    If successful, you should see the NVIDIA GPU table again, which confirms Docker can access the GPU.

8. Kaito's node estimator reads the `nvidia.com/gpu.memory` label to calculate how many nodes are needed. If you were on WSL, WSL2 GPU Feature Discovery may not auto-populate all labels. Ensure the node has at least:
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

## 4. Install Adaptive App Capability Portfolio (Min-AI)

Start with the `mi-ai` portfolio Helm chart. The first bundled component is Keycloak.

1. Create namespace:

    ```bash
    kubectl create namespace min-ai
    ```

2. Install the `min-ai` portfolio from the local chart:

    ```bash
    helm install min-ai ./charts/portfolios/min-ai --namespace min-ai --set min.nameOverride=min-ai
    ```

3. Verify deployments:

    ```bash
    kubectl get pods -n min-ai
    kubectl get svc -n min-ai
    ```

    You should see services like:

    ```bash
    NAME                         TYPE        
    min-ai-keycloak              ClusterIP   
    min-ai-keycloak-discovery    ClusterIP
    min-ai-keycloak-postgresql   ClusterIP 
    ```

4. In a separate Terminal, expose Keycloak with port-forward (keep this terminal running):

    ```bash
    kubectl port-forward -n min-ai svc/min-ai-keycloak 8080:8080
    ```

5. Open a browser and navigate to `localhost:8080`. Log in to KeyCloak portal with user `admin` and password `admin` (which are defined in the `values.yaml` for the Helm chart).

6. Click on "Clients" in the left pane, and click on the "Create Client" button to create a new client. Set up a name for the client and accept all default values across screens except for:

    * `Cleint authentication`: set to **On**.
    * `Valid redirect URIs`: set to `http://localhost:3000/*` (or more specifically `http://localhost:3000/auth/oidc/callback`). 
    
    Click "Save" to save the client definition.

7. Go to "Credentials" tab and copy the client secret. You'll need both client id and secret for the next step.

## 5. Install the app

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
    --parameters oidcIssuer=http://min-keycloak.min.svc.cluster.local:8080/realms/master \
    --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
    --parameters oidcBrowserAuthEndpoint=http://localhost:8080/realms/master/protocol/openid-connect/auth \
    --parameters oidcTokenEndpoint=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/token \
    --parameters oidcUserInfoEndpoint=http://min-keycloak.min.svc.cluster.local:8080/realms/master/protocol/openid-connect/userinfo \
    --parameters oidcClientId=<Keycloak client id> \
    --parameters oidcClientSecret=<Keycloak client secret> \
    --parameters aiProvider=local \
    --parameters aiModel=Qwen/Qwen3-0.6B
    ```

    > **NOTE:** The Keycloak service is `ClusterIP`, which is ideal for in-cluster calls from the frontend pod. This setup uses two different URLs: Browser redirects to `http://localhost:8080` (via `oidcBrowserAuthEndpoint`); Token and userinfo requests go to the in-cluster `min-keycloak.min.svc.cluster.local` (via explicit `oidcTokenEndpoint` and `oidcUserInfoEndpoint`). This causes an issuer mismatch: Keycloak issues a token with iss claim set to `http://localhost:8080/realms/master` (the URL used during authentication), but the frontend validates the token against `oidcIssuer=http://min-keycloak.min.svc.cluster.local:8080/realms/master` by default. Use `oidcIssuerOverride=http://localhost:8080/realms/master` to tell the frontend which issuer to expect. In production, Keycloak is typically deployed behind an ingress with a single DNS name used everywhere, avoiding this split-URL issue. See [keycloak-ingress.md](../../../docs/authentication/keycloak-ingress.md) for setup details.

    If you want to use an Azure OpenAI deployment endpoint, you need to set these parameters accordingly:

    ```bash
    --parameters aiProvider=azure-key \
    --parameters aiModelName=gpt-4 \
    --parameters aiEndpoint=https://antho-openai.openai.azure.com/ \
    --parameters aiApiKey=<Azure OpenAI service deployment key>
    ```

    > **NOTE:** Your GPU size limits which models you can use. For a 4G memory GPU, the only feasible model seems to be `Qwen/Qwen3-0.6B`. For slightly bigger GPU, you can try `ministral-3-3b-instruct`. See troubleshoot guide #5 below.

2. Expose the frontend:

    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

3. Open the app at `http://localhost:3000`.
4. Login using local account admin/admin, or click on "Sign in with OIDC" button to use KeyCloak to login with federated credential.

This is intentionally independent of the app model in `radius/app.bicep`. The app stays portable; the environment decides whether service-to-service traffic is meshed.

## 6. Clean up

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
5. You can trick Kaito scheduler by overclaiming the `nvidia.com/gpu.memory` label to make it think your GPU has bigger memory. For example, by labeling a 4GiB GPU to `12288`, Kaito can be tricked to schedule `Qwen/Qwen3-0.6B` on a single node, which seems to work for the demo.

6. Manully labeling the node seems to lead Kaito GPU feature discovery pod to crash. This doesn't appear to affect the demo flow.

7. If Radius got stuck at a "The target resource is in progress state: Updating." conflict, the best known approch to restore is to re-create the Kubernetes cluster.

8. To get a list of models supported by the currently installed version of Kaito:

    ```bash
    kubectl get cm kaito-supported-models -n kaito-workspace -o yaml
    ```
## Additional Topics

* [Deploy Keycloak behind an ingress](../../../docs/authentication/keycloak-ingress.md)
* [Configure KeyCloak federation with Azure Entra ID](../../../docs/authentication/keycloak-entra.md)
* [Configure Keycloak federation with a local Active Directory](../../../docs/authentication/keycloak-active-directory.md)
* [Configure credential sync from local Active Directory to an Azure Entra tenant](../../../docs/authentication/microsoft-entra-connect.md)