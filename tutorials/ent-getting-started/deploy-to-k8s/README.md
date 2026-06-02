# Deploy Adaptive App (Ent) to Local K8s

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
    kubectl create namespace ent
    ```

2. Install the `core` portfolio from the local chart:

    ```bash
    helm install ent ./charts/portfolios/ent --namespace ent
    ```

3. Verify deployments:

    ```bash
    kubectl get all -n ent 
    kubectl get all -n istio-system    
    ```

    You should see services like:

    ```bash
    NAME                      TYPE          NAMESPACE  
    ent-keycloak              ClusterIP     ent
    ent-keycloak-discovery    ClusterIP     ent
    ent-keycloak-postgresql   ClusterIP     ent
    istiod                    ClusterIP     istio-system
    opa                       ClusterIP     ent
    otel-collector            ClusterIP     ent
    prometheus                ClusterIP     ent
    zipkin                    ClusterIP     ent
    ```

4. In a separate Terminal, expose Keycloak with port-forward (keep this terminal running):

    ```bash
    kubectl port-forward -n ent svc/ent-keycloak 8080:8080
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

8. Build the OIDC env vars for the next section's `rad deploy`. The chart's `ent-oidc` ConfigMap already exposes the in-cluster endpoint URLs; the client ID/secret come straight from Keycloak, and the browser endpoint is the port-forward URL:

    ```bash
    eval "$(kubectl -n ent get cm ent-oidc -o go-template='
    export OIDC_ISSUER={{ .data.issuer | printf "%q" }}
    export OIDC_AUTH_ENDPOINT={{ .data.authEndpoint | printf "%q" }}
    export OIDC_TOKEN_ENDPOINT={{ .data.tokenEndpoint | printf "%q" }}
    export OIDC_USERINFO_ENDPOINT={{ .data.userInfoEndpoint | printf "%q" }}
    ')"
    export OIDC_CLIENT_ID=$OIDC_APP_ID
    export OIDC_CLIENT_SECRET=$OIDC_APP_SECRET
    export OIDC_BROWSER_AUTH_ENDPOINT=http://localhost:8080/realms/master/protocol/openid-connect/auth
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

    > **NOTE:** The OIDC values above are read from the chart-rendered `ent-oidc` ConfigMap. Users reach Keycloak via `kubectl port-forward` on `localhost:8080`, while the frontend pod calls it via the in-cluster Service DNS, so Keycloak issues tokens with `iss=http://localhost:8080/realms/master` (the URL used during browser auth). `oidcIssuerOverride=http://localhost:8080/realms/master` makes the frontend accept tokens validated against the localhost issuer while still hitting the token/userinfo endpoints in-cluster. In production, expose Keycloak behind an ingress with a single DNS name to avoid this split-URL setup — see [keycloak-ingress.md](../../../docs/authentication/keycloak-ingress.md).

    If you want to use an Azure OpenAI deployment endpoint, you need to set these parameters accordingly:

    ```bash
    --parameters aiProvider=azure-key \
    --parameters aiModelName=gpt-4 \
    --parameters aiEndpoint=https://antho-openai.openai.azure.com/ \
    --parameters aiApiKey=<Azure OpenAI service deployment key>
    ```
2. Enable Istio sidecar injection on the app namespace.

    Radius creates the app namespace (`trading-<app-name>`), but it does not label it for Istio sidecar injection. The `enableIstioInjection=true` parameter on `app.bicep` only adds the `sidecar.istio.io/inject: "true"` pod annotation, which is ignored unless the namespace is enrolled in the mesh. Label the namespace and restart the workloads so they come back with sidecars:

    ```bash
    export APP_NAMESPACE=trading-portable-apps
    kubectl label namespace $APP_NAMESPACE istio-injection=enabled --overwrite
    kubectl rollout restart deployment -n $APP_NAMESPACE
    ```

    >**NOTE:** The `core` Helm chart (inherited by `ent`) handles the cluster-wide mTLS plumbing automatically: the pre-install hook installs Istio (`istio-base` + `istiod`) into `istio-system`, and the post-install hook applies a mesh-wide `PeerAuthentication` with `mtls.mode: STRICT` in the Istio root namespace. Labeling each app namespace is left to the operator because Radius owns app-namespace creation. The chart also deploys observability components (OpenTelemetry collector, Prometheus, and Zipkin) in the `core` namespace, and the app automatically sends telemetry to the collector.

3. (Optional) Observe mTLS

    ```bash
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

This separation allows apps to remain portable; the environment (Helm chart) decides whether observability and mTLS are available.

4. Expose the frontend:

    ```bash
    rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
    ```

5. Open the app at `http://localhost:3000`.
6. Login using local account admin/admin, or click on "Sign in with OIDC" button to use KeyCloak to login with federated credential.

## 5. Try an OPA authorization policy

The `ent` portfolio deployment in this tutorial sets `enableGovernance=true`
on the Radius app, which materializes a `Radius.Resources/governance`
resource (`trading-governance`). The Kubernetes OPA recipe deploys an OPA
(Open Policy Agent) instance into the app namespace and registers it with
Istio as a per-app extension provider named
`opa-ext-authz-grpc-trading-governance`. Out of the box OPA ships a
default-allow Rego policy, so no requests are blocked. In this step you'll
attach a `CUSTOM` `AuthorizationPolicy` to the `frontend` workload and
update the Rego policy to reject any request that carries an `x-deny: true`
header — a small but end-to-end demonstration of mesh-level external
authorization.

1. Confirm Istio sees this app's OPA as an extension provider:

    ```bash
    kubectl -n istio-system get cm istio -o jsonpath='{.data.mesh}' | grep -A3 extensionProviders
    ```

    You should see an entry named `opa-ext-authz-grpc-trading-governance`
    pointing at `trading-governance-opa.trading-portable-apps.svc.cluster.local`.

2. Tell Istio to delegate authorization for the `frontend` pods to OPA. A
   ready-made manifest is provided at
   [`frontend-ext-authz.yaml`](frontend-ext-authz.yaml) (namespace is
   `trading-portable-apps`):

    ```bash
    kubectl apply -f frontend-ext-authz.yaml
    ```

3. Replace the chart's default-allow Rego with the demo policy in
   [`opa-policy.rego`](opa-policy.rego), which denies any request carrying
   `x-deny: true`:

    The recipe creates the policy ConfigMap as `<resourceName>-opa-policy`
    in the app's namespace (here: `trading-governance-opa-policy` in
    `trading-portable-apps`):

    ```bash
    kubectl -n trading-portable-apps create configmap trading-governance-opa-policy \
      --from-file=policy.rego=opa-policy.rego \
      --dry-run=client -o yaml | kubectl apply -f -

    # OPA picks up ConfigMap changes once the projected volume refreshes
    # (typically < 60s). Restart the pod to apply immediately:
    kubectl -n trading-portable-apps rollout restart deployment/trading-governance-opa
    kubectl -n trading-portable-apps rollout status  deployment/trading-governance-opa
    ```

4. Exercise the policy. The `kubectl port-forward` exposed in step 3
   (used by `rad resource expose`) tunnels straight into the pod's
   container port and **bypasses the Istio sidecar**, so requests from
   `localhost:3000` never reach OPA. To see the policy take effect, send
   the request from a pod inside the mesh. The application containers
   don't ship `curl`, so launch a long-lived `curlimages/curl` pod in the
   same namespace (which gets a sidecar via namespace-wide injection)
   and `kubectl exec` into it. A one-shot `kubectl run ... -- curl ...`
   pod would race the sidecar — curl exits before Envoy finishes its
   initial xDS push, so the request never makes it out:

    ```bash
    # Start a sleeping curl pod with the sidecar (wait for 2/2 Ready):
    kubectl -n trading-portable-apps run curlbox \
      --image=curlimages/curl:8.10.1 --restart=Never \
      --command -- sleep infinity
    kubectl -n trading-portable-apps wait --for=condition=Ready pod/curlbox --timeout=60s

    # Allowed — no x-deny header. Expect HTTP/1.1 302 (redirect to /login.html).
    kubectl -n trading-portable-apps exec curlbox -c curlbox -- \
      curl -sI http://frontend:3000/

    # Denied — Envoy returns 403 because OPA evaluated allow=false.
    kubectl -n trading-portable-apps exec curlbox -c curlbox -- \
      curl -sI -H 'x-deny: true' http://frontend:3000/
    ```

    The second request should respond with `HTTP/1.1 403 Forbidden`
    (Istio's standard `CUSTOM`-action denial — the body would contain
    `RBAC: access denied` if you drop `-I` and request the body).

5. (Optional) Tail OPA decision logs to watch each verdict in real time:

    ```bash
    kubectl -n trading-portable-apps logs -f deployment/trading-governance-opa | grep decision_id
    ```

6. Clean up the demo policy when you're done so the rest of the tutorial
   continues to work normally:

    ```bash
    kubectl -n trading-portable-apps delete authorizationpolicy frontend-ext-authz
    kubectl -n trading-portable-apps delete pod curlbox --ignore-not-found
    # Re-deploying the app re-applies the recipe's default-allow policy.
    # To revert in place without a redeploy, recreate the ConfigMap from
    # the recipe's baked-in Rego or simply delete it and let the next
    # `rad deploy` recreate it:
    kubectl -n trading-portable-apps delete configmap trading-governance-opa-policy
    kubectl -n trading-portable-apps rollout restart deployment/trading-governance-opa
    ```

> **NOTE:** Real policies typically key off identity (JWT claims, SPIFFE
> identities pushed in by Istio, etc.) and request attributes such as path
> and method. The `x-deny` header here is only a teaching aid. See the
> [OPA Envoy plugin docs](https://www.openpolicyagent.org/docs/latest/envoy-introduction/)
> for the full `input` schema and richer examples.

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

## Additional Topics

* [Deploy Keycloak behind an ingress](../../../docs/authentication/keycloak-ingress.md)
* [Configure KeyCloak federation with Azure Entra ID](../../../docs/authentication/keycloak-entra.md)
* [Configure Keycloak federation with a local Active Directory](../../../docs/authentication/keycloak-active-directory.md)
* [Configure credential sync from local Active Directory to an Azure Entra tenant](../../../docs/authentication/microsoft-entra-connect.md)