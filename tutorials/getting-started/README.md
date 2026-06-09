# Adaptive Apps — Getting Started

This tutorial walks you through the step of deploying an Adaptive Apps portfolio (`min`, `core`,
`ent`, `min-ai`, `core-ai`, `ent-ai`) to any supported environment
(local k3s, AKS, Azure Local, Azure Arc), and then deploy a sample Radius application.

## 0. Prerequisites

* [docker](https://docs.docker.com/)
* [helm](https://helm.sh/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
* [rad](https://docs.radapp.io/guides/tooling/rad-cli/howto-rad-cli/)
* (optional) An [OpenAI API Key](https://platform.openai.com/api-keys) or [Azure OpenAI](https://azure.microsoft.com/en-us/products/ai-foundry/models/openai) deployment key, when you plan to use cloud-based LLM.


## 1. Choose your portfolio

Pick a portfolio and export it. **Every subsequent step references `$PORTFOLIO`** — re-export it in any new terminal you open.

```bash
export PORTFOLIO=min          # or: core | ent | min-ai | core-ai | ent-ai
export RELEASE=$PORTFOLIO     # Helm release name; defaults to the portfolio
export NAMESPACE=$PORTFOLIO   # Kubernetes namespace for the portfolio
```

If your target is AKS, define a few more environment variables:
```bash
export AZURE_SUBSCRIPTION=<your Azure subscrption id>
export RESOURCE_GROUP=<Azure resource group>
export AKS_CLUSTER=<AKS cluster name>
```

| Portfolio | Identity | Service mesh (Istio) | Observability | Governance (OPA) | On-cluster AI (Kaito) |
| --- | :-: | :-: | :-: | :-: | :-: |
| `min` | ✓ | ✗ | ✗ | ✗ | ✗ |
| `core` | ✓ | ✓ | ✓ | ✗ | ✗ |
| `ent` | ✓ | ✓ | ✓ | ✓ (off by default) | ✗ |
| `min-ai` | ✓ | ✗ | ✗ | ✗ | ✓ |
| `core-ai` | ✓ | ✓ | ✓ | ✗ | ✓ |
| `ent-ai` | ✓ | ✓ | ✓ | ✓ (off by default) | ✓ |

> **What "On-cluster AI" means here.** The `*-ai` suffix only controls whether the cluster provisions Kaito so the sample app can serve a model **locally**. It does **not** restrict which AI provider the app uses at runtime — any portfolio (including `min`) can be pointed at OpenAI / Azure OpenAI via the `aiProvider=openai`. Pick `*-ai` only if you want a self-contained on-cluster model.

## 2. Bring up the platform

This section gets you from "empty cluster context" to "Radius is installed,
the portfolio chart is running, and OIDC env vars are exported". Pick **one**:

| Option | What it covers | When to use |
| --- | --- | --- |
| OPTION 1 — `ada` CLI | Helm install **+** Radius install **+** Keycloak OIDC client provisioning. On `--platform k3s` it also creates a local k3d cluster. | Recommended for all platforms. On k8s / AKS / Arc / Azure Local you prepare the cluster yourself first, then point `ada` at the existing `kubectl` context. |
| OPTION 2 — Manual | Same steps, one shell command at a time | When you want to see what's happening, or to script just one of the sub-steps. |

### 2.1 OPTION 1 — Use the `ada` CLI

1. Set up the CLI per [`common/prepare-cli.md`](../common/prepare-cli.md).

2. Pick the `--platform` matching your target environment. For everything
   except `k3s`, you must have already prepared a cluster and have
   `kubectl` pointed at it (see the prep guides linked in OPTION 2 below).

   | `--platform` | Cluster auto-provisioned? | Notes |
   | --- | --- | --- |
   | `k3s` | yes (k3d) | one-command end-to-end on your workstation |
   | `k8s` | no | bring-your-own standard Kubernetes (local or on cloud) |
   | `aks` | no | adds the managed-Istio helm overrides |
   | `arc` | no | passthrough — standard kubectl context |
   | `azure-local` | no | passthrough — standard kubectl context |

3. Run:

    ```bash
    export PLATFORM=k3s   # or: k8s | aks | arc | azure-local

    ada bootstrap \
      --portfolio $PORTFOLIO \
      --platform $PLATFORM \
      --release $RELEASE \
      --namespace $NAMESPACE \
      --chart-root charts \
      --with-radius \
      --keep-port-forward
    ```

    If your are using AKS, you need to pass in three additional parameters:
    `--azure-subscription`, `--resource-group`, and `--aks-cluster`. `ada`
    uses the rg/cluster pair to discover the active Istio add-on revision
    and (with `--with-radius`) to federate the Radius service accounts.

    ```bash
    ada bootstrap \
      --portfolio $PORTFOLIO \
      --platform aks \
      --release $RELEASE \
      --namespace $NAMESPACE \
      --chart-root charts \
      --with-radius \
      --azure-subscription $AZURE_SUBSCRIPTION \
      --resource-group $RESOURCE_GROUP \
      --aks-cluster $AKS_CLUSTER \
      --keep-port-forward
    ```

    What that single command does:

    1. **Cluster** — on `--platform k3s` only, creates (or reuses) a
    local k3d cluster and switches your `kubectl` context to it. On every
    other platform this step is a no-op — `ada` uses whatever cluster the
    current `kubectl` context points at. *For AI portfolios you also need to
    run [`common/prepare-gpu-kaito.md`](../common/prepare-gpu-kaito.md)
    before invoking `ada bootstrap`.*
    2. **Portfolio chart** — `helm upgrade --install` with the right
    `charts/adaptive-apps/profiles/<portfolio>.yaml` profile. On `aks` it
    also adds the managed-Istio overrides.
    3. **Radius** — installs the control plane, creates the `adaptive`
    workspace and group, the `trading` environment, registers the project's
    resource types, and deploys `radius/local-env.bicep` (or
    `aks-env.bicep` on `--platform aks`).
    4. **OIDC client** — port-forwards to Keycloak, creates/reuses the
    `adaptive-apps` OIDC client, prints an `export OIDC_*` block, and (with
    `--keep-port-forward`) holds the port-forward open in the foreground.

4. Copy the `export OIDC_*` lines into the terminal you'll use for step 3 and
skip ahead to [3 Deploy the sample app](#3-deploy-the-sample-app).

### 2.2 OPTION 2 — Manual

#### 2.2.1 Prepare the cluster

Pick **one** environment and follow the linked guide. Each leaves you with
`kubectl` pointed at a working cluster:

| Environment | Guide |
| --- | --- |
| Local k3s (via k3d) | [`common/prepare-k3s.md`](../common/prepare-k3s.md) |
| Azure Kubernetes Service (AKS) | [`common/prepare-aks.md`](../common/prepare-aks.md) |
| Azure Arc-enabled cluster | [`common/prepare-arc.md`](../common/prepare-arc.md) |
| Azure Local | [`common/prepare-azure-local.md`](../common/prepare-azure-local.md) |

AI portfolios (`*-ai`) additionally need GPU + Kaito on the cluster. Follow
[`common/prepare-gpu-kaito.md`](../common/prepare-gpu-kaito.md) before
proceeding.

#### 2.2.2 Install the portfolio chart

Use `helm install` to install the chart, parametrized by a profile
file that turns on the right capabilities for `$PORTFOLIO`:

```bash
helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  -n $NAMESPACE --create-namespace
```

On AKS, add the managed-Istio overrides (the chart must *not* install Istio;
the AKS add-on owns it):

```bash
helm install $RELEASE charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/$PORTFOLIO.yaml \
  --set features.istio.install=false \
  --set istio.namespace=aks-istio-system \
  -n $NAMESPACE --create-namespace
```

Verify:

```bash
kubectl get pods -n $NAMESPACE
```

#### 2.2.3 Create an OIDC client in Keycloak

Open Keycloak and create the OIDC client the sample app will use:

```bash
kubectl port-forward -n $NAMESPACE svc/$RELEASE-keycloak 8080:8080
```

In another terminal, open <http://localhost:8080>, log in as `admin` / `admin`
(or whatever you set in `keycloak.admin.*`), then under realm `master`:

1. **Clients → Create client**
   * `Client type`: OpenID Connect
   * `Client ID`: `adaptive-apps`
2. **Capability config**
   * `Client authentication`: **On**
   * `Standard flow`: enabled
3. **Login settings**
   * `Valid redirect URIs`: `http://localhost:3000/*`
4. Save, then go to the **Credentials** tab and copy the client secret.

Export the OIDC env vars (you'll use them in §3):

```bash
export OIDC_CLIENT_ID=adaptive-apps
export OIDC_CLIENT_SECRET=<paste-from-credentials-tab>
export OIDC_ISSUER=http://$RELEASE-keycloak.$NAMESPACE.svc.cluster.local:8080/realms/master
export OIDC_AUTH_ENDPOINT=$OIDC_ISSUER/protocol/openid-connect/auth
export OIDC_TOKEN_ENDPOINT=$OIDC_ISSUER/protocol/openid-connect/token
export OIDC_USERINFO_ENDPOINT=$OIDC_ISSUER/protocol/openid-connect/userinfo
export OIDC_BROWSER_AUTH_ENDPOINT=http://localhost:8080/realms/master/protocol/openid-connect/auth
```

#### 2.2.4 Install Radius

```bash
rad install kubernetes --set rp.publicEndpointOverride=localhost:8081

rad workspace create kubernetes trading --context "$(kubectl config current-context)" --force
rad workspace switch trading

rad group create adaptive
rad group switch adaptive

rad env create trading --group adaptive --namespace trading-adaptive-apps
rad env switch trading
```

> **AKS only — register the Azure credential.** The `wi-helper.sh` you ran
> in [`prepare-aks.md`](../common/prepare-aks.md) created an Entra app named
> `$AKS_CLUSTER-radius-app` and federated it to the Radius service
> accounts. Now bind it to the Radius control plane (`ada bootstrap
> --with-radius --platform aks` does this for you):
>
> ```bash
> export APPLICATION_CLIENT_ID=$(az ad app list \
>   --display-name "$AKS_CLUSTER-radius-app" --query '[].appId' -o tsv)
> export TENANT_ID=$(az account show --query tenantId -o tsv)
>
> rad credential register azure wi \
>   --client-id $APPLICATION_CLIENT_ID --tenant-id $TENANT_ID
>
> # Verify (may take 30+ seconds to refresh):
> rad credential show azure
> ```

Register the resource types used by the sample app:

```bash
rad resource-type create -f radius/resource-types/types.yaml
```

Deploy the local environment bicep so recipes are registered with Radius:

```bash
rad deploy radius/local-env.bicep --group adaptive --environment trading
```

> **AKS** uses `radius/aks-env.bicep` instead of `local-env.bicep`.

---

## 3. Deploy the sample app

### 3.0 (AKS only) Provision per-app workload identities

The sample app's `backend` and `frontend` containers need Azure managed
identities federated to their Kubernetes service accounts. Run
`app-wi-setup.sh` once per workload — it creates the managed identity,
federates it, and assigns the needed RBAC. Skip this section on every
other platform.

```bash
cd tutorials/getting-started/assets

./app-wi-setup.sh backend  $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER trading default
./app-wi-setup.sh frontend $RESOURCE_GROUP $AZURE_SUBSCRIPTION $AKS_OIDC_ISSUER trading default

export BACKEND_CLIENT_ID=$(az identity show -g $RESOURCE_GROUP -n backend  --query clientId -o tsv)
export FRONTEND_CLIENT_ID=$(az identity show -g $RESOURCE_GROUP -n frontend --query clientId -o tsv)
cd -
```

> Run this **once** per cluster. Re-deploying the app later does not
> re-provision identities. Pass `$BACKEND_CLIENT_ID` /
> `$FRONTEND_CLIENT_ID` to `rad deploy radius/app.bicep` as parameters
> wired to the bicep's workload-identity inputs.

### 3.1 Pick an AI provider

Pick how the sample app's AI agent reaches a model. The choice is
**independent of the portfolio** — any portfolio can use either provider:

* **`local`** — talk to a Kaito-served model running in the cluster. Only
  works on `*-ai` portfolios (they're the ones that provision Kaito).
* **`openai`** — call an OpenAI or Azure OpenAI endpoint with the API key
  from §0. Works on every portfolio.

```bash
# Local on-cluster model (requires a *-ai portfolio):
AI_PARAMS=(--parameters aiProvider=local --parameters aiModel=Qwen/Qwen3-0.6B)

# OR — OpenAI / Azure OpenAI (works with any portfolio):
AI_PARAMS=(--parameters aiProvider=openai --parameters aiModelName=gpt-4o --parameters aiApiKey=<OpenAI API Key>)
```

> **Agent guardrails (AI portfolios only).** When `PORTFOLIO=ent-ai`
> the in-pod [Agent Governance Toolkit](https://microsoft.github.io/agent-governance-toolkit/)
> sidecar is the recommended default — opt in by appending
> `--parameters enableAgentGuardrails=true` to the `rad deploy` below.
> The sidecar scans every inbound prompt for prompt-injection attempts
> before it reaches the model. It also works with `min-ai` and `core-ai`
> on an opt-in basis. The recipe defaults to the Microsoft-published
> [`ghcr.io/microsoft/agentmesh/governance-sidecar`](https://github.com/microsoft/agent-governance-toolkit/pkgs/container/agentmesh%2Fgovernance-sidecar)
> image. See [docs/portfolios/overview.md](../../docs/portfolios/overview.md#features-by-portfolio)
> for the per-portfolio matrix. The guardrails parameter is a no-op on
> non-AI portfolios.

Deploy:

```bash
rad deploy radius/app.bicep \
  --group adaptive \
  --environment trading \
  --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
  --parameters imageTag=latest \
  --parameters authUsername=admin \
  --parameters authPassword=admin \
  --parameters otelCollectorEndpoint=http://otel-collector.$NAMESPACE:4318 \
  --parameters oidcIssuer=$OIDC_ISSUER \
  --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
  --parameters oidcAuthEndpoint=$OIDC_AUTH_ENDPOINT \
  --parameters oidcBrowserAuthEndpoint=$OIDC_BROWSER_AUTH_ENDPOINT \
  --parameters oidcTokenEndpoint=$OIDC_TOKEN_ENDPOINT \
  --parameters oidcUserInfoEndpoint=$OIDC_USERINFO_ENDPOINT \
  --parameters oidcClientId=$OIDC_CLIENT_ID \
  --parameters oidcClientSecret=$OIDC_CLIENT_SECRET \
  "${AI_PARAMS[@]}"
```

**Istio portfolios only** (`core`, `ent`, `core-ai`, `ent-ai`) — the app
namespace isn't enrolled in the mesh yet, so pods come up without sidecars.
That's fine for getting the app running; enroll the namespace when you're
ready to exercise mTLS, `AuthorizationPolicy`, and traffic shaping by
following the [service mesh tutorial](../service-mesh/README.md).

Expose the frontend and open it:

```bash
rad resource expose Applications.Core/containers frontend -a adaptive-apps --port 3000 --remote-port 3000
```

Browse to <http://localhost:3000>. Log in with the local admin account
(`admin` / `admin`), or click **Sign in with OIDC** to authenticate through
Keycloak.

> **`aiProvider=local`:** The Kaito-managed model pod may take several
> minutes to become ready while the model is downloaded. Watch
> `kubectl get pods -n trading-adaptive-apps`.

---

## 4. Clean up

```bash
rad app delete adaptive-apps
helm uninstall $RELEASE -n $NAMESPACE
kubectl delete namespace $NAMESPACE

# Local k3s only (uses the `k3s` cluster name created by `--platform k3s`):
k3d cluster delete k3s
```

---

## 5. Next steps

Portfolio-specific operational tutorials build on this baseline:

* **Service mesh** (`core`, `ent`, `core-ai`, `ent-ai`): authoring
  `AuthorizationPolicy` and `VirtualService` resources, mTLS troubleshooting.
  See [Service mesh — operator tutorial](../service-mesh/README.md).
* **Governance** (`ent`, `ent-ai`): enabling OPA via
  `--set features.governance.opa=true --set features.governance.istioExtAuthz=true`
  and writing Rego policies. See [Governance (OPA + Istio) — operator tutorial](../governance/README.md).
* **AI** (`*-ai`): swapping Kaito models, sizing GPU memory, switching between
  local and cloud `aiProvider`. *(tutorial coming)*
* **Observability**: querying traces in Zipkin and metrics in Prometheus,
  wiring up an external Grafana. *(tutorial coming)*
