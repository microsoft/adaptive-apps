# Radius Deployment Guide — portable-apps

This directory contains all Radius assets needed to deploy the **portable-apps** stock-trading simulator on Kubernetes.

```
radius/
├── app.bicep                          # Radius application model (no AI agent)
├── app-with-ai.bicep                  # Radius application model (includes AI agent)
├── local-env.bicep                    # Environment for local k3s (Kaito AI recipe)
├── aks-env.bicep                      # Environment for AKS / Azure (Azure OpenAI recipe)
├── bicepconfig.json                   # Bicep extension references
├── resource-types/
│   └── types.yaml                     # All custom types under Radius.Resources
└── recipes/
    ├── postgres/
    │   └── kubernetes-trading-postgres.bicep   # PostgreSQL 16 + trading schema
    ├── mqtt/
    │   └── kubernetes-mosquitto.bicep          # Eclipse Mosquitto 2 (MQTT + WS)
    └── ai-agent/
        ├── kubernetes-kaito.bicep              # Kaito LLM (local/AKS + GPU)
        └── azure-openai.bicep                  # Azure OpenAI account + deployment
```

---

## Prerequisites

| Tool | Install |
|------|---------|
| `rad` CLI | https://docs.radapp.io/installation/ |
| `kubectl` | https://kubernetes.io/docs/tasks/tools/ |
| A running Kubernetes cluster | [k3d](https://k3d.io) recommended for local dev |
| Access to GHCR images | Ensure your cluster can pull from `ghcr.io/microsoft/adaptive-apps` (or set `imageRegistry`/`imageTag` to your own published images) |
| `az bicep` / Bicep CLI | https://learn.microsoft.com/azure/azure-resource-manager/bicep/install |

---

## Step 1 — Install Radius on your cluster

```bash
rad install kubernetes
```

Verify:

```bash
rad version
kubectl get pods -n radius-system
```

---

## Step 2 — Register the Resource Types

Radius needs to know about the custom portable resource types before they can be
referenced in `app.bicep`. All three types are defined in the single `types.yaml` file
and can be registered with one command:

```bash
cd radius/

rad resource-type create --from-file resource-types/types.yaml
```

---

## Step 3 — Generate the Bicep extension

Bicep needs type information to validate and auto-complete the custom resource
types. A single bundle is generated from the consolidated `types.yaml` and covers
the unified namespace (`Radius.Resources`):

```bash
cd radius/

rad bicep publish-extension --from-file resource-types/types.yaml --target types.tgz
```

> **Note:** `bicepconfig.json` references `types.tgz` via the single `radiusResources` extension.

---

## Step 4 — Create the group, workspace, and Environment

Create a dedicated Kubernetes namespace, Radius group, and workspace. The
Environment itself — including all Recipe registrations — is created
declaratively by deploying `local-env.bicep` in the next step.

```bash
kubectl create namespace trading

rad group create trading
rad workspace create kubernetes trading \
  --group trading \
  --context $(kubectl config current-context)
```

---

## Step 5 — Deploy the Environment

Deploy `local-env.bicep` to create the Radius Environment and register all
Recipes in one step. Recipes are pre-published to GHCR by the CI pipeline
([`.github/workflows/publish-recipes.yml`](../.github/workflows/publish-recipes.yml))
— no manual publishing required.

```bash
cd radius/

rad deploy local-env.bicep --group trading
```

Verify the environment and its recipes:

```bash
rad environment show trading
rad recipe list --environment trading
```

> **AKS / Azure deployments** use `aks-env.bicep` instead — see
> [Deploying to Azure](#deploying-to-azure) below.

---

## Step 6 — Identify your deployment parameters

Container images are built and published by CI
([`.github/workflows/build-and-publish.yml`](../.github/workflows/build-and-publish.yml)),
so you can use the default registry/tag values unless you intentionally override them.

Gather the values you will pass on the command line in Step 7 (`authPassword` is the only required value without a default):

| Parameter | Description | Local dev default |
|-----------|-------------|-------------------|
| `imageRegistry` | Container registry prefix | `ghcr.io/microsoft/adaptive-apps` |
| `authPassword` | Password for the local frontend login | *(required — no default)* |

---

## Step 7 — Deploy the application (without AI)

```bash
cd radius/
rad deploy app.bicep --group trading --environment trading --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps --parameters imageTag=latest --parameters authUsername=admin --parameters authPassword=<your-password>
```

This single command:
1. Runs the PostgreSQL Recipe → deploys Postgres + seeds the trading schema
2. Runs the Mosquitto Recipe → deploys the MQTT broker
3. Deploys the `backend` and `frontend` containers with all
   environment variables wired up from the Recipe outputs
4. The frontend proxies all browser traffic to backend and MQTT
   — only port 3000 needs to be exposed

### Optional: deploy with AI agent enabled

```bash
cd radius/
rad deploy app-with-ai.bicep --group trading --environment trading --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps --parameters imageTag=latest --parameters authUsername=admin --parameters authPassword=<your-password> --parameters aiModel=Qwen/Qwen3-0.6B
```

Monitor deployment:

```bash
rad app graph -a portable-apps
rad resource list Applications.Core/containers -a portable-apps
```

---

## Step 8 — Access the application

The frontend server proxies all API, AI-agent, and MQTT WebSocket traffic, so
you only need to expose one port:

```bash
rad resource expose Applications.Core/containers frontend -a portable-apps --port 3000 --remote-port 3000
```

Then open **http://localhost:3000** and log in with the `authUsername` / `authPassword` you passed above.

---

## Deploying to Azure

For Azure deployments with AI, use `app-with-ai.bicep`. Instead of a local Kaito
LLM, the AI recipe provisions an Azure OpenAI account.

### Azure prerequisites

| Tool | Install |
|------|---------|
| Azure CLI (`az`) | https://learn.microsoft.com/cli/azure/install-azure-cli |
| An AKS cluster | Or any Kubernetes cluster with access to Azure |
| A service principal | Radius uses it to provision Azure resources |

### Azure Step 1 — Create an AKS cluster (if you don't have one)

```bash
az group create -n <resource-group> -l eastus
az aks create -g <resource-group> -n <cluster-name> --node-count 1 --generate-ssh-keys
az aks get-credentials -g <resource-group> -n <cluster-name> --context aks-trading
```

This stores the AKS credentials under a named context (`aks-trading`) so it
doesn't overwrite your local k3s context. Switch between them with:

```bash
kubectl config use-context aks-trading    # → AKS
kubectl config use-context default        # → local k3s (use your actual k3s context name)
kubectl config get-contexts               # list all contexts
```

> **GPU drivers on AKS:** AKS GPU node pools (e.g. `Standard_NC` series) come
> with NVIDIA drivers and the device plugin **pre-installed**. You do **not**
> need to install the NVIDIA device plugin, configure the containerd runtime, or
> label nodes — AKS handles all of this automatically. The WSL2-specific vLLM
> workarounds (`gpu-memory-utilization`, `enforce-eager`, `runtimeClassName`)
> are also not needed on AKS.

### Azure Step 2 — Install Radius + register types + generate extension

Steps 1–3 are identical to the local workflow:

```bash
rad install kubernetes
# rad install kubernetes --set global.azureWorkloadIdentity.enabled=true

cd radius/
rad resource-type create --from-file resource-types/types.yaml
rad bicep publish-extension --from-file resource-types/types.yaml --target types.tgz
```

### Azure Step 3 — Register Azure credentials

Create a service principal and register it so Radius can provision Azure resources.
The `rad credential register` command must target the **AKS workspace** — switch
to it first if you also have a local workspace.

```bash
az ad sp create-for-rbac -n radius-sp --role Contributor \
  --scopes /subscriptions/<subscription-id>/resourceGroups/<resource-group>

# Create a workspace pointing at the AKS context (no group yet)
rad workspace create kubernetes aks-trading --context aks-trading
rad workspace switch aks-trading

rad credential register azure \
  --client-id <appId> \
  --client-secret <password> \
  --tenant-id <tenant>
```

### Azure Step 4 — Create group, workspace, and deploy the Environment

> **Important:** `rad group create` and `rad workspace create` talk to the
> Radius instance on the **current workspace's** cluster. Make sure
> `rad workspace switch aks-trading` was run first, otherwise these commands
> target your local k3s Radius instance.

```bash
kubectl create namespace trading

# Create the Radius group on the AKS Radius instance
rad group create trading

# Update the workspace to use the group
rad workspace create kubernetes aks-trading \
  --group trading \
  --context aks-trading

cd radius/
rad deploy aks-env.bicep \
  --parameters azureSubscriptionId=<subscription-id> \
  --parameters azureResourceGroup=<resource-group>
```

### Azure Step 5 — Deploy the application

```bash
rad deploy app-with-ai.bicep --group trading --environment aks-trading \
  --parameters authPassword=<your-password> \
  --parameters aiModel=gpt-4o
```

> The only difference from the local deploy is the environment name
> (`aks-trading` vs `trading`) and `aiModel=gpt-4o` (or another
> Azure OpenAI model) instead of `Qwen/Qwen3-0.6B`.

### Azure Step 6 — Access the application

Same as the local workflow:

```bash
rad resource expose Applications.Core/containers frontend \
  -a portable-apps --port 3000 --remote-port 3000
```

Or configure an Ingress / Load Balancer for production access.

---

## Tear down

```bash
rad app delete portable-apps
rad resource delete Applications.Core/environments trading --group trading
rad group delete trading
```

---

## Architecture reference

```
┌──────────────────────────────────────────────────────────────────────┐
│  Radius Application: portable-apps                                   │
│                                                                      │
│  ┌──────────┐ /api/* ┌──────────┐  PostgreSQL  ┌────────────────┐  │
│  │ frontend │───────▶│ backend  │─────────────▶│  trading-db    │  │
│  │  :3000   │        │  :8080   │  MQTT        │  Postgres :5432│  │
│  │ (proxy)  │        └──────────┘─────────────▶├────────────────┤  │
│  │          │/advice                            │  trading-mqtt  │  │
│  │          │───────▶┌──────────┐              │  Mosquitto     │  │
│  │          │/mqtt   │ ai-agent │              │  :1883 / :9001 │  │
│  │          │──ws──▶ │  :7000   │              └────────────────┘  │
│  └──────────┘        └──────────┘  ◀── Radius.Resources/aiModels  │
│   (only :3000           │               recipe                    │
│    exposed)             │  CONNECTION_AI_*  (auto-injected)        │
│                     ┌───▼────────────────────┐                     │
│                     │  trading-ai             │                     │
│                     │  Radius.Resources/      │                     │
│                     │  aiModels               │                     │
│                     └────────────────────────┘                      │
└──────────────────────────────────────────────────────────────────────┘
```

### Resource Type → Recipe mapping

| Resource Type | Recipe | Description |
|---------------|--------|-------------|
| `Radius.Resources/postgreSqlDatabases` | `kubernetes-trading-postgres.bicep` | PostgreSQL 16-alpine with custom init SQL that creates the trading schema and seeds a demo account |
| `Radius.Resources/mqttBrokers` | `kubernetes-mosquitto.bicep` | Eclipse Mosquitto 2 with both a plain TCP listener (1883) and a WebSocket listener (9001) |
| `Radius.Resources/mqttBrokers` | `azure-event-grid.bicep` | Azure Event Grid Namespace with MQTT topic spaces enabled (secure MQTT/TLS endpoint for Azure environments) |
| `Radius.Resources/workloadIdentities` | `local-noop.bicep` | Local stub workload identity contract for non-Azure environments |
| `Radius.Resources/workloadIdentities` | `azure-workload-identity.bicep` | User-assigned managed identity + federated credential + service account annotation + Event Grid TopicSpaces RBAC |
| `Radius.Resources/aiModels` | `kubernetes-kaito.bicep` | Open-source LLM served in-cluster via the Kaito operator (AKS + GPU). Outputs `provider=local`. |
| `Radius.Resources/aiModels` | `azure-openai.bicep` | Azure OpenAI account + model deployment. Outputs `provider=openai` and an API key. |

---

## Troubleshooting

**Recipe not found during deploy**
Check the recipe is registered: `rad recipe list --environment trading`.
Ensure the OCI image was published and is accessible from within the cluster.

**Database tables missing after deploy**
The init SQL runs only on a *fresh* PostgreSQL data directory. If the pod
already has data, delete the Deployment and its PVC to force re-initialisation:
```bash
kubectl delete deployment/postgres-<hash> -n trading
```

**Browser cannot reach backend / MQTT**
The frontend server proxies all backend, AI-agent, and MQTT WebSocket traffic.
Check the frontend pod logs for proxy errors:
```bash
kubectl logs -n <namespace> -l radapp.io/resource=frontend --tail=30
```
Ensure the in-cluster service names (`backend:8080`, `ai-agent:7000`, and the
mosquitto service) are reachable from the frontend pod.

**AI model pod not starting / checking model download progress**
The Kaito Workspace creates a StatefulSet pod that downloads the model and runs
vLLM. To check its status and logs:
```bash
# Find the inference pod (named after the Workspace resource)
kubectl get pods -n <namespace> -l kaito.sh/workspace=<workspace-name>

# Stream logs to watch model download and vLLM startup
kubectl logs -n <namespace> <workspace-pod-name> -f
```
If the Workspace shows `RESOURCEREADY: False` with message "Not enough Nodes are
ready", this is likely the Kaito v0.9.0 node estimator bug (it ignores
`max-model-len` from the ConfigMap and over-estimates GPU requirements). Fix by
inflating the `nvidia.com/gpu.memory` node label — see the
[root README known issues](../README.md#5-known-issues-kaito-v090) for details.

**Bicep type errors**
Make sure `types.tgz` has been generated (Step 3) and is present in the `radius/`
directory alongside `bicepconfig.json`.
