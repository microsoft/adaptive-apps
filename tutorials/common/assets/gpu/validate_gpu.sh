#!/usr/bin/env bash
set -euo pipefail

NODE_NAME="${NODE_NAME:-k3d-localk8s-server-0}"

echo "Checking NVIDIA runtime inside k3d node..."
docker exec -it "$NODE_NAME" sh -c '
  which nvidia-container-runtime
  which nvidia-ctk
  ls -l /dev/dxg /dev/nvidia* 2>/dev/null || true
  grep -i nvidia /var/lib/rancher/k3s/agent/etc/containerd/config.toml
'

echo
echo "Checking NVIDIA device plugin..."
kubectl get pods -n kube-system -l name=nvidia-device-plugin-ds
kubectl logs -n kube-system -l name=nvidia-device-plugin-ds --tail=50

echo
echo "Checking node GPU capacity..."
kubectl describe node "$NODE_NAME" | grep -A20 -E "Capacity|Allocatable|nvidia.com/gpu"

echo
echo "Running CUDA pod test..."
kubectl delete pod nvidia-runtime-test --ignore-not-found=true

cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: nvidia-runtime-test
spec:
  runtimeClassName: nvidia
  restartPolicy: Never
  containers:
  - name: cuda
    image: nvidia/cuda:12.3.2-base-ubuntu22.04
    command: ["nvidia-smi"]
    resources:
      limits:
        nvidia.com/gpu: 1
EOF

kubectl wait --for=condition=Ready pod/nvidia-runtime-test --timeout=120s || true
kubectl logs nvidia-runtime-test
kubectl delete pod nvidia-runtime-test