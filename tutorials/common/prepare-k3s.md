# Prepare a K3s Cluster


1. Install k3d:


    ```bash
    curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

    ```

2.  Verify installation:

    ```bash
    k3d --version
    ```

3. Create a K3s cluster:

    ```bash
    k3d cluster create localk8s
    # Set K3D_FIX_DNS=0 helps cluster creation complete in environments where it otherwise stalls at configuring CoreDNS configmap
    ```
    
