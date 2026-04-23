# Portable Apps Getting Started Tutorial


## Prepare a local Kubernetes cluster
To demostrate local deployments, you need a local Kubernetes cluster such as [k3s](https://github.com/rancher/k3s) or [Kind](https://kind.sigs.k8s.io/), or a full-scale Kubernetes cluster. We'll use k3s in this tutorial.

### Prerequisites

* [docker](https://docs.docker.com/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
* [rad](https://docs.radapp.io/guides/tooling/rad-cli/howto-rad-cli/)

### Enable NVIDIA Docker support
1. Enable NVIDIA Docker runtime
    ```bash
    curl -fsSL https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
    sudo apt-get update
    sudo apt-get install -y nvidia-container-toolkit
    sudo nvidia-ctk runtime configure --runtime=containerd
    sudo systemctl restart containerd
    ```
2. Verify NVIDIA GPU is accessble in Docker:
    ```bash
    docker run --rm --gpus all nvidia/cuda:12.3.2-base-ubuntu22.04 nvidia-smi
    ```
    This should show something like:
    ```bash
    Wed Feb 18 17:20:13 2026
    +-----------------------------------------------------------------------------------------+
    | NVIDIA-SMI 570.188                Driver Version: 573.71         CUDA Version: 12.8     |
    |-----------------------------------------+------------------------+----------------------+
    | GPU  Name                 Persistence-M | Bus-Id          Disp.A | Volatile Uncorr. ECC |
    | Fan  Temp   Perf          Pwr:Usage/Cap |           Memory-Usage | GPU-Util  Compute M. |
    |                                         |                        |               MIG M. |
    |=========================================+========================+======================|
    |   0  NVIDIA RTX A2000 Laptop GPU    On  |   00000000:F3:00.0 Off |                  N/A |
    | N/A   59C    P3             11W /   33W |       0MiB /   4096MiB |      0%      Default |
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
### Installation steps

> **NOTE:** For detailed instructions please see [Radius doc](https://docs.radapp.io/guides/operations/kubernetes/overview/#supported-kubernetes-clusters).

1. Install k3s:

    ```bash
    curl -sfL https://get.k3s.io | sh -
    ```    

2. Verify installation:

    ```bash
    k3d --version
    ```
3. Create a K3s cluster:

    ```bash
    k3d cluster create -p "8081:80@loadbalancer" --k3s-arg "--disable=traefik@server:*" --k3s-arg "--disable=servicelb@server:*"  --gpus all
    ```

    * The `-p`parameter adds a port mapping which routes traffic from the local machine into the cluster.
    * The `--disable=traefik` parameter disables [traefik](https://doc.traefik.io/traefik/) pods because Radius provides an ingress controller.
    * The `--disable=servicelb` parameter disables the k3d internal load balancer.
    * The `--gpus 1` parameter add the first GPU to the cluster node.

4. Intall NVIDIA GPU device plugin for Kubernetes:
    ```bash
    kubectl create -f https://raw.githubusercontent.com/NVIDIA/k8s-device-plugin/v0.17.1/deployments/static/nvidia-device-plugin.yml
    ```
5. Patch NVIDIA daemonset to use `nivida` runtime (instead of `runc`):
    ```bash
    kubectl -n kube-system patch ds nvidia-device-plugin-daemonset --type='json' -p='[{"op":"add","path":"/spec/template/spec/runtimeClassName","value":"nvidia"}]'
    kubectl -n kube-system rollout restart ds nvidia-device-plugin-daemonset
    ```
5. Verify the K3s node is avaialble for GPU scheduling:

     ```bash
    kubectl get nodes -o jsonpath='{range .items[*]}{.metadata.name}{"  gpu="}{.status.allocatable.nvidia\.com/gpu}{"\n"}{end}'
    ```
   
    This should show something like:
    ```bash
    <node name>  gpu=1
    ```


4. Install Radius control plane:
    ```bash
    rad install kubernetes --set rp.publicEndpointOverride=localhost:8081
    rad init # answer No when asked if you want to initialize an app under the current folder
    ```

## Install Kaito on the local Kubernetes cluster

To demostrate agents using local langague models, you need to install Kaito to the local cluster. Kaito requires GPU-enabled nodes. Please see intructions [here](./enable-nvidia-docker.md) for details on enabling GPU access for Docker.

1. Install Kaito workspace controller:
    ```bash
    export CLUSTER_NAME=kaito

    helm repo add kaito https://kaito-project.github.io/kaito/charts/kaito
    helm repo update
    helm upgrade --install kaito-workspace kaito/workspace \
    --version 0.9.0 \
    --namespace kaito-workspace \
    --create-namespace \
    --set clusterName="$CLUSTER_NAME" \
    --set featureGates.disableNodeAutoProvisioning=true \
    --set nvidiaDevicePlugin.enabled=false \
    --set localCSIDriver.useLocalCSIDriver=false \
    --wait
    ```

    > **NOTE:** `featureGates.disableNodeAutoProvisioning=true` disables node auto provisioning on local cluster. When running on WSL, you need to disable localCSI with `localCSIDriver.useLocalCSIDriver=false`. `nvidiaDevicePlugin.enabled=false` avoids conflict if you already have the NVIDIA device plugin installed. See the [root README](../../README.md#kaito-on-wsl2--k3s--setup-notes) for additional WSL2 workarounds.
2. Verify 
    ```bash
    kubectl get pods -n kaito-workspace
    kubectl describe deploy kaito-workspace -n kaito-workspace
    ```
   