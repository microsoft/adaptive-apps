# Adaptive Apps — Getting Started

This tutorial deploys an Adaptive Apps portfolio and the trading sample to
local k3s, Kubernetes, AKS, Azure Arc-enabled Kubernetes, or Azure Local.

The recommended path uses `ada` for platform installation and Radius
configuration. If you need to own each Helm, Keycloak, and Radius command, use
the [advanced manual setup](advanced-manual.md) instead.

## Workflow

| Frequency | Preparation or operation |
| --- | --- |
| Once per workstation | Install tools and trust the local development CA if HTTPS is needed. |
| Once per cluster | Prepare Kubernetes and enable only the required capabilities: KAITO, portable-agent storage/networking, or other platform features. |
| Once per clone or reviewed configuration change | Build Helm dependencies, generate the local certificate files, create `agents.json`, and run `ada init` when portable agents are selected. |
| Every deployment or update | Run `ada bootstrap`, run the generated `rad deploy` command, and verify the application. |

## 1. One-time preparation

### 1.1 Prepare the CLI and Kubernetes cluster

1. Install `ada` by following [Prepare the CLI](../common/prepare-cli.md).
2. Complete the common steps in
   [Prepare Kubernetes](../common/prepare-kubernetes.md). That guide separates
   baseline cluster preparation from optional KAITO, locally trusted HTTPS,
   and portable-agent capabilities. With `--platform k3s`, `ada` can create
   the local cluster for you.
3. If using `SAMPLE=trading-native`, also complete the guide's
   [portable-agent section](../common/prepare-kubernetes.md#6-optional-enable-kubernetes-hosted-portable-agents).

### 1.2 Choose the sample and portfolio

Set these variables in every new terminal:

```bash
# Standard trading sample:
export SAMPLE=trading
export PORTFOLIO=min          # min | core | ent | min-ai | core-ai | ent-ai

# OR trading with Kubernetes-hosted portable agents:
export SAMPLE=trading-native
export PORTFOLIO=min-ai       # portable agents require an AI portfolio

export RELEASE=$PORTFOLIO
export NAMESPACE=$PORTFOLIO
export PLATFORM=k3s           # k3s | k8s | aks | arc | azure-local
```

`k3s` creates or reuses a local k3d cluster. Every other platform uses the
current `kubectl` context and does not create a cluster.

| Portfolio | Identity | Istio | Observability | Governance | AI capability |
| --- | :-: | :-: | :-: | :-: | :-: |
| `min` | ✓ | ✗ | ✗ | ✗ | ✗ |
| `core` | ✓ | ✓ | ✓ | ✗ | ✗ |
| `ent` | ✓ | ✓ | ✓ | ✓ | ✗ |
| `min-ai` | ✓ | ✗ | ✗ | ✗ | ✓ |
| `core-ai` | ✓ | ✓ | ✓ | ✗ | ✓ |
| `ent-ai` | ✓ | ✓ | ✓ | ✓ | ✓ |

The AI capability does not install or select KAITO. A local model requires the
separate GPU and KAITO preparation linked from the cluster preparation guide.

For AKS, also set:

```bash
export AZURE_SUBSCRIPTION=<subscription-id>
export RESOURCE_GROUP=<resource-group>
export AKS_CLUSTER=<aks-cluster>
export AKS_OIDC_ISSUER=$(az aks show \
  --resource-group "$RESOURCE_GROUP" \
  --name "$AKS_CLUSTER" \
  --query oidcIssuerProfile.issuerUrl \
  -o tsv)
```

### 1.3 Configure portable agents

Skip this section for `SAMPLE=trading`.

Choose a model configuration. This example uses public OpenAI. Create a file
outside the repository that contains only the OpenAI API key, restrict the file
to its owner (mode `0600` on Linux or macOS), and set
`OPENAI_API_KEY_FILE` to its location:

```bash
export OPENAI_API_KEY_FILE=/path/to/openai.key

ada agents-config \
  --output agents.json \
  --import-images \
  --with-openai-model \
  --openai-model gpt-4o \
  --openai-api-key-file "$OPENAI_API_KEY_FILE"
```

Other supported starting points:

```bash
# Identities and agents without a model:
ada agents-config --output agents.json --import-images

# Local model, only after GPU and KAITO preparation:
ada agents-config \
  --output agents.json \
  --import-images \
  --with-kaito-model
```

`ada agents-config` resolves immutable recipes from the current branch's
tracked GitHub remote, falling back to `origin`. Use
`--repository OWNER/REPOSITORY` only to select another repository explicitly.

Confirm that the trusted certificate files described in
[Prepare Kubernetes](../common/prepare-kubernetes.md#5-optional-prepare-locally-trusted-https)
exist, then generate retained private inputs:

```bash
ada init \
  --agents-config agents.json \
  --namespace "$NAMESPACE" \
  --release "$RELEASE" \
  --refresh-derived-state
```

Run `ada init` again only when reviewed configuration changes require new
derived inputs. Explicit identities, credentials, and state remain retained;
they are not regenerated or pruned automatically.

### 1.4 AKS workload identities

On AKS, provision the application's backend and frontend workload identities
once per cluster:

```bash
cd tutorials/getting-started/assets
./app-wi-setup.sh backend \
  "$RESOURCE_GROUP" "$AZURE_SUBSCRIPTION" "$AKS_OIDC_ISSUER" trading default
./app-wi-setup.sh frontend \
  "$RESOURCE_GROUP" "$AZURE_SUBSCRIPTION" "$AKS_OIDC_ISSUER" trading default
cd -

export BACKEND_CLIENT_ID=$(az identity show \
  -g "$RESOURCE_GROUP" -n backend --query clientId -o tsv)
export FRONTEND_CLIENT_ID=$(az identity show \
  -g "$RESOURCE_GROUP" -n frontend --query clientId -o tsv)
```

Keep those client IDs for the application deployment. Skip this section on
other platforms.

## 2. Deploy or update

### 2.1 Bootstrap the platform

For the standard sample:

```bash
ada bootstrap \
  --portfolio "$PORTFOLIO" \
  --platform "$PLATFORM" \
  --release "$RELEASE" \
  --namespace "$NAMESPACE" \
  --chart-root charts \
  --with-radius \
  --keep-port-forward
```

For portable agents:

```bash
ada bootstrap \
  --portfolio "$PORTFOLIO" \
  --platform "$PLATFORM" \
  --release "$RELEASE" \
  --namespace "$NAMESPACE" \
  --chart-root charts \
  --with-radius \
  --with agents \
  --agents-config agents.json \
  --keep-port-forward \
  --demo-users
```

For AKS, add:

```text
--azure-subscription "$AZURE_SUBSCRIPTION"
--resource-group "$RESOURCE_GROUP"
--aks-cluster "$AKS_CLUSTER"
```

`ada bootstrap` is repeatable. It:

1. Creates local k3s only when `--platform k3s` is selected.
2. Installs or updates the selected portfolio.
3. Installs Radius and reconciles the Radius environments and recipes.
4. For portable agents, installs the pinned Envoy Gateway and agentgateway,
   applies retained credentials, and provisions the Keycloak client.
5. Prints the application deployment values.

Keep this terminal running when `--keep-port-forward` is used. For portable
agents it exposes the same Kubernetes Gateway at:

* <https://localhost:3443> — trading UI
* <https://localhost:8444> — Keycloak

### Running local and AKS deployments together

Each deployment needs distinct workstation ports. For the AKS bootstrap, add:

```text
--keycloak-port 8445
--trading-port 3445
```

These forward local `8445` to the Keycloak Gateway listener `8444`, and local
`3445` to the trading Gateway listener `3443`. The Kubernetes listener ports
do not change. The Keycloak override also updates its public hostname, native
OIDC issuers, and the application exports printed by bootstrap; changing only
a port-forward would leave those URLs inconsistent.

For repeatable preparation, set `"keycloakPort": 8445` in the AKS agents
configuration. Alternatively, pass `--keycloak-port 8445` to both `ada init`
and `ada bootstrap`. The default remains `8444`. Keep the frontend callback
origin in the same configuration consistent with how you expose the remote
frontend:

* Gateway HTTPS on `3445`: `"tradingOrigin": "https://localhost:3445"`.
* Direct HTTP port-forward on `3001`: `"tradingOrigin": "http://localhost:3001"`.

The `--trading-port` flag changes forwarding only, not `tradingOrigin`.
For an existing deployment, review these URL changes and add
`--refresh-derived-state` when preparing/bootstraping to replace derived
inputs. Retained passwords, client secrets, certificates, and state are not
regenerated. Run the new complete `rad deploy` command printed by bootstrap
to update the frontend's OIDC settings as well. Existing sessions may require
signing in again after the issuer URL changes.

Use separate configuration files and `--state-dir` directories for independent
deployments. For an existing installation, keep using its original retained
state; do not switch it to an empty directory. Use separate browser profiles
because cookies for `localhost` are shared across ports. Stop old forwards in
their owning terminals before restarting bootstrap, and explicitly select the
intended Kubernetes context before each deployment.

### 2.2 Deploy the application

#### Portable-agent sample

Copy the complete exports and `rad deploy radius/app.bicep` command printed by
`ada bootstrap` into a second terminal and run it without editing the generated
private parameter file.

The deployment creates the trading frontend's cross-namespace `HTTPRoute` to
the platform's HTTPS Gateway. The frontend callback origin and Keycloak client
origin both come from `agents.json`.

#### Standard sample

Choose one AI binding:

```bash
# Local KAITO model:
AI_PARAMS=(--parameters aiProvider=local --parameters aiModel=Qwen/Qwen3-0.6B)

# OR OpenAI:
# AI_PARAMS=(--parameters aiProvider=openai --parameters aiModelName=gpt-4o --parameters aiApiKey=<key>)
```

Use the OIDC exports printed by `ada bootstrap`, then deploy:

```bash
rad deploy radius/app.bicep \
  --group adaptive \
  --environment trading \
  --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
  --parameters imageTag=latest \
  --parameters authUsername=admin \
  --parameters authPassword=admin \
  --parameters "otelCollectorEndpoint=http://otel-collector.$NAMESPACE:4318" \
  --parameters "oidcIssuer=$OIDC_ISSUER" \
  --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
  --parameters "oidcAuthEndpoint=$OIDC_AUTH_ENDPOINT" \
  --parameters "oidcBrowserAuthEndpoint=$OIDC_BROWSER_AUTH_ENDPOINT" \
  --parameters "oidcTokenEndpoint=$OIDC_TOKEN_ENDPOINT" \
  --parameters "oidcUserInfoEndpoint=$OIDC_USERINFO_ENDPOINT" \
  --parameters "oidcClientId=$OIDC_CLIENT_ID" \
  --parameters "oidcClientSecret=$OIDC_CLIENT_SECRET" \
  "${AI_PARAMS[@]}"
```

On AKS, add the workload-identity parameters:

```text
--parameters backendClientId="$BACKEND_CLIENT_ID"
--parameters frontendClientId="$FRONTEND_CLIENT_ID"
```

## 3. Verify

For the optional **Trading Advisor** memory portability demo (independent of
the Agents tab), follow [advisor memory setup](../../src/ai-agent/README.md).
It adds AKS PostgreSQL writes (or managed-identity Azure Tables), one-way PostgreSQL replication,
and identity-scoped local recall; it is disabled by default.

For portable agents:

```bash
kubectl -n "$NAMESPACE" get statefulset,pvc,httproute,securitypolicy
kubectl -n trading get httproute,securitypolicy
```

Browse to <https://localhost:3443>, sign in through Keycloak, select
**Agents**, open an agent, invoke it, and open Monitor.

For the standard sample, expose the frontend:

```bash
rad resource expose Applications.Core/containers frontend \
  -a adaptive-apps \
  --port 3000 \
  --remote-port 3000
```

Browse to <http://localhost:3000>. Local password users can use the trading
pages but cannot access portable-agent routes.

For a local KAITO model, the model Pod may take several minutes to become
ready while downloading model data:

```bash
kubectl get pods -n trading
```

## 4. Clean up

```bash
rad app delete adaptive-apps
helm uninstall "$RELEASE" -n "$NAMESPACE"
kubectl delete namespace "$NAMESPACE"

# Local k3s only:
k3d cluster delete k3s
```

Deleting the trading application does not delete retained portable-agent
identities or state. Delete the entire disposable local cluster only when you
intend a full reset.

## 5. Advanced and operational guides

* [Advanced manual setup](advanced-manual.md)
* [Portable agents on Radius](../../docs/agents/native-radius.md)
* [Service mesh tutorial](../service-mesh/README.md)
* [Governance tutorial](../governance/README.md)
* [Portfolio overview](../../docs/portfolios/overview.md)
