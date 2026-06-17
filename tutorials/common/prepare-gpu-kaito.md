# Prepare GPU + Kaito on K3s

These steps are only required for the `*-ai` portfolios when you intend to run
local LLM inference on-cluster (i.e. `aiProvider=local`). If you plan to use a
cloud provider such as OpenAI or Azure OpenAI (`aiProvider=openai`), skip this
doc entirely.

> **Note:** This guide assumes you have already followed
> [prepare-k3s.md](./prepare-k3s.md) up to (but not including) the
> `k3d cluster create` step. We re-create the cluster here with GPU passthrough.

## Prerequisites

* NVIDIA GPU with >= 4G memory, preferably >= 16G
* Latest NVIDIA driver installed (with WSL support if using WSL)

## 1. Create the GPU-enabled K3s cluster

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

> **NOTE:** To build the `hbai/cuda:0.1` image yourself, use
> [`assets/gpu/Dockerfile.nvidia`](./assets/gpu/Dockerfile.nvidia):
> `docker build -t hbai/cuda:0.1 -f tutorials/common/assets/gpu/Dockerfile.nvidia .`

## 2. Install Kyverno

Kaito needs a cluster policy to patch StatefulSets with the NVIDIA
`runtimeClassName`:

```bash
helm repo add kyverno https://kyverno.github.io/kyverno/
helm repo update

helm upgrade --install kyverno kyverno/kyverno \
  -n kyverno \
  --create-namespace
```

## 3. Apply the GPU bootstrap artifact

```bash
kubectl apply -f tutorials/common/assets/gpu/gpu_bootstrap.yaml
```

## 4. Validate the node has allocatable GPU

```bash
./tutorials/common/assets/gpu/validate_gpu.sh
```

## 5. Label the node with GPU facts

Kaito's node estimator reads `nvidia.com/gpu.memory` to size workloads. WSL2
GPU Feature Discovery may not auto-populate all labels, so set them manually:

```bash
export NODE_NAME=k3d-localk8s-server-0   # adjust via `kubectl get nodes`
kubectl label node $NODE_NAME nvidia.com/gpu=true
kubectl label node $NODE_NAME nvidia.com/gpu.product=<your-gpu-product>
kubectl label node $NODE_NAME nvidia.com/gpu.present=true
kubectl label node $NODE_NAME nvidia.com/gpu.count=1
kubectl label node $NODE_NAME nvidia.com/gpu.memory=<MiB>
```

> **Tip:** If you have a small GPU, you can over-claim `nvidia.com/gpu.memory`
> (e.g. label a 4GiB GPU as `12288`) to coax the Kaito estimator into scheduling
> small models like `Qwen/Qwen3-0.6B` on a single node.

## 6. Install Kaito

```bash
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

> **NOTE:** Minimum required Kaito version is 0.9.0. `nvidiaDevicePlugin.enabled=false`
> avoids conflict with the device-plugin DaemonSet bundled in step 3.
> `disableNodeAutoProvisioning=true` enables BYO GPU mode.

## Known issues (Kaito v0.9.0)

| Issue | Workaround |
| --- | --- |
| Webhook panic (`MustParse("")`) when applying a generic-model Workspace in BYO mode | `kubectl delete validatingwebhookconfiguration validation.workspace.kaito.sh` |
| Node estimator ignores `max-model-len` from the ConfigMap and over-estimates `targetNodeCount` | Inflate the `nvidia.com/gpu.memory` node label to satisfy the estimator |
