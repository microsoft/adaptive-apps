# Deploy Adaptive App to Local K8s

## 0. Prerequisites

* [docker](https://docs.docker.com/)
* [Helm](https://helm.sh/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
* [rad](https://docs.radapp.io/guides/tooling/rad-cli/howto-rad-cli/)


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
    rad deploy local-env.bicep --group trading
    rad environment list --group trading
    ```

## 3. Install Adaptive App Capability Portfolio

Start with the `compute-core` portfolio Helm chart. The first bundled component is Keycloak.

1. Create namespace:

    ```bash
    kubectl create namespace compute-core
    ```

2. Install the `compute-core` portfolio from the local chart:

    ```bash
    helm install compute-core ./charts/portfolios/compute-core --namespace compute-core
    ```

3. Verify deployments:

    ```bash
    kubectl get pods -n compute-core
    kubectl get svc -n compute-core
    ```

    You should see services like:

    ```bash
    NAME                               TYPE        
    compute-core-keycloak              ClusterIP   
    compute-core-keycloak-discovery    ClusterIP
    compute-core-keycloak-postgresql   ClusterIP 
    ```


## 4. Install the app

Deploy the non-AI app model to the Radius environment created above.

1. Deploy the app:

    ```bash
    cd radius
    rad deploy app.bicep \
      --group trading \
      --environment trading \
      --parameters imageRegistry=ghcr.io/haishi2016/portable-apps \
      --parameters imageTag=latest \
      --parameters authUsername=admin \
      --parameters authPassword=admin \
      --parameters oidcIssuer=http://compute-core-keycloak.compute-core.svc.cluster.local:8080/realms/master \
      --parameters oidcAuthEndpoint=http://compute-core-keycloak.compute-core.svc.cluster.local:8080/realms/master/protocol/openid-connect/auth \
      --parameters oidcTokenEndpoint=http://compute-core-keycloak.compute-core.svc.cluster.local:8080/realms/master/protocol/openid-connect/token \
      --parameters oidcUserInfoEndpoint=http://compute-core-keycloak.compute-core.svc.cluster.local:8080/realms/master/protocol/openid-connect/userinfo \
      --parameters oidcClientId=<KeyCloak client id> \
      --parameters oidcClientSecret=<KeyCloak client secret>
    ```
2. Add an entry in your hosts file to map service hostname to callback address. 

    ```bash
    127.0.0.1 compute-core-keycloak.compute-core.svc.cluster.local
    ```

    > **NOTE**: This is needed when the KeyCloak service is configured as a ClusterIP service. This should be optimized in future versions.

2. Expose the frontend:

    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

3. Open the app at `http://localhost:3000`.
4. Login using local account admin/admin, or click on "Sign in with OIDC" button to use KeyCloak to login with federated credential.