# Enable NVIDIA runtime for Docker on WSL

Before you can add a GPU-enabled node to k3s (or k3d), Docker inside WSL must be able to access the NVIDIA GPU. This guide walks through enabling NVIDIA GPU support for Docker in an Ubuntu-based WSL environment.

## Prerequisites

* NVIDIA GPU on the Windows host
* Latest NVIDIA Windows driver installed (with WSL support)
* Docker Engine installed inside WSL
* systemd enabled in WSL

## Installation

1. First, verify GPU works inside WSL:
    ```bash
    nvidia-smi
    ```
    You should see the GPU table.
    If this fails, teh Windows driver or WSL GPU integration is not working and must be fixed first.
2. Install NVIDIA Container Toolkit:
    ```bash
    curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey \
    | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg

    curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list \
    | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \
    | sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list

    sudo apt-get update
    sudo apt-get install -y nvidia-container-toolkit
    ```
3. Configure Docker to use NVIDIA runtime
    ```bash
    sudo nvidia-ctk runtime configure --runtime=docker # this modifies /etc/docker/daemon.json automatically
    sudo systemctl restart docker
    ```
4. Test CUDA container:
    ```bash
    docker run --rm --gpus all nvidia/cuda:12.3.2-base-ubuntu22.04 nvidia-smi
    ```
    If successful, you should see the NVIDIA GPU table again, which confirms Docker can access the GPU.
