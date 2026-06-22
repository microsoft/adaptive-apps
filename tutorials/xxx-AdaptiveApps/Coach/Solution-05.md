# Challenge 05 - Port the App Across Environments - Coach's Guide

[< Previous Solution](./Solution-04.md) - **[Home](./README.md)** - [Next Solution >](./Solution-06.md)

## Notes & Guidance

This challenge is where the portability promise becomes visible. In Challenges 3 and 4, teams created a stable application-facing contract (`Radius.Resources/*`) and then registered different recipes behind that contract. Challenge 5 proves the point by deploying the same `radius/app.bicep` application model to a second environment and letting the environment select the backing services.

- The learning objective is **not** "deploy the app twice." The learning objective is that teams can explain why the second deployment requires only environment and parameter changes, not application model changes.
- Expected time: **45-75 minutes** if both environments were prepared in earlier challenges. Add more time if a second cluster, Azure credentials, or Azure resource group still need to be provisioned.
- Keep teams focused on the distinction between:
  - **Application definition**: `radius/app.bicep`, owned by the app team, should stay unchanged.
  - **Environment definition**: `radius/local-env.bicep` or `radius/aks-env.bicep`, owned by the platform team, chooses recipes and provider scope.
  - **Deployment parameters**: image tag, password, AI model, Azure resource scope, and optional feature flags.
- This challenge should continue from the objects created in the earlier challenges:
  - Challenge 2 created the Radius workspaces, environments, and resource groups.
  - Challenge 3 registered the portable `Radius.Resources/*` resource type contracts.
  - Challenge 4 registered recipes for the sample `env-local` and `env-azure` environments.
- The strongest demo is local/Azure Local first, then AKS/Azure second. The app model remains the same, but Postgres, MQTT, workload identity, and AI can all land on different implementations.
- If teams only have one cluster available, they can still practice the pattern by creating a second Radius environment and deploying to a different Radius group. Coach them that this demonstrates the control-plane model, but a real production/non-production split should use separate clusters or at least separate namespaces and cloud scopes.
- Do not let teams solve the challenge by copying `app.bicep` and hard-coding environment-specific values into it. That defeats the purpose of Radius portability.

## Key Concepts - What Actually Moves?

Before teams run commands, spend a few minutes drawing the boundary between the portable app and the platform-specific environment.

```
Same application model                    Different environment recipes
----------------------                    -----------------------------
radius/app.bicep                          radius/local-env.bicep
  adaptive-apps                             PostgreSQL container
  trading-db       ---------------->        Mosquitto container
  trading-mqtt                              Keycloak container
  backend/frontend                          Kaito or in-cluster AI

radius/app.bicep                          radius/aks-env.bicep
  adaptive-apps                             PostgreSQL recipe (same today)
  trading-db       ---------------->        Azure Event Grid MQTT
  trading-mqtt                              Azure workload identity
  backend/frontend                          Azure OpenAI
```

The application asks for capabilities, not products:

- `Radius.Resources/postgreSqlDatabases` instead of "create this exact PostgreSQL deployment."
- `Radius.Resources/mqttBrokers` instead of "use Mosquitto" or "use Event Grid."
- `Radius.Resources/workloadIdentities` instead of "wire this cloud-specific identity primitive."
- `Radius.Resources/aiModels` instead of "this workload must use Kaito" or "this workload must use Azure OpenAI."

The environment decides which recipe fulfils each capability. Not every capability must have a different recipe in every environment; the current repository keeps PostgreSQL on the Kubernetes recipe in both environment files while swapping MQTT, workload identity, and AI in the Azure environment. That is still the portability boundary: the app model asks for `postgreSqlDatabases`, and the environment decides how to satisfy it.

### Naming handoff from earlier challenges

Challenge 2 introduces domain-oriented names such as `ws-azure-prod`, `ws-local-prod`, `env-azure-prod`, and `rg-finance`. Challenge 4's prebuilt environment files use the repository sample names `rg-trading` for the Radius group and `env-local` / `env-azure` for the application environments/namespaces. Do not let naming distract from the portability lesson.

Use this guide with either naming style:

| Earlier challenge object | If teams followed the sample artifact path | If teams used domain names from Challenge 2 |
|---|---|---|
| First workspace | `ws-local-prod` | `ws-local-prod`, `ws-local-nonprod`, etc. |
| Second workspace | `ws-azure-prod` | `ws-azure-prod`, `ws-azure-nonprod`, etc. |
| Radius group | `rg-trading` | `rg-finance`, `rg-trading`, etc. |
| Environment name | `env-local-prod` / `env-azure-prod` | `env-azure-prod`, `env-local-prod`, etc. |
| Application file | `radius/app.bicep` | `radius/app.bicep` |

For the command examples below, the guide uses Challenge 4's sample names (`rg-trading` group and `env-local` / `env-azure` environments) because those match the prebuilt `radius/local-env.bicep` and `radius/aks-env.bicep` flow. If a team used Challenge 2's business names, substitute their group and environment names consistently.

## Solution Guide

### Stage 1 - Confirm the first environment is ready

Start from an environment completed in Challenge 4. Do not recreate the control plane, resource types, or recipes here; Challenge 5 is about reusing them. The examples below use the Challenge 4 sample group `rg-trading` and environment `env-local-prod`.

```bash
rad workspace switch <first-workspace>
rad group switch rg-trading

rad environment show env-local-prod
rad recipe list --environment env-local-prod
```

The recipe list should include the portable resource types registered by the environment. The application uses a subset directly, and optional features such as AI, governance, and guardrails activate additional types based on deployment parameters.

| Resource type | Local / Azure Local recipe outcome |
|---|---|
| `Radius.Resources/postgreSqlDatabases` | PostgreSQL container with the trading application schema |
| `Radius.Resources/mqttBrokers` | Eclipse Mosquitto container |
| `Radius.Resources/idProviders` | Keycloak OIDC provider |
| `Radius.Resources/workloadIdentities` | Local Kubernetes service account / no-op identity |
| `Radius.Resources/aiModels` | Kaito in-cluster LLM when AI is enabled |
| `Radius.Resources/governance` | OPA policy decision point when governance is enabled |
| `Radius.Resources/agentGuardrails` | Agent Governance Toolkit sidecar support when guardrails are enabled |

If recipes are missing, send the team back to Challenge 4. Challenge 5 depends on the environment having the recipe mappings in place.

### Stage 2 - Deploy the app to the first environment

Use the same application Bicep the team will later deploy elsewhere:

```bash
rad deploy radius/app.bicep \
    --group rg-trading \
    --environment env-local-prod \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=<your-password>
```

If the team is validating AI portability, enable the recipe-backed AI resource:

```bash
rad deploy radius/app.bicep \
    --group rg-trading \
    --environment env-local-prod \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=<your-password> \
    --parameters aiProvider=local \
    --parameters aiModel=qwen2.5-coder-7b-instruct
```

Validate the app model and backing resources:

```bash
rad app graph -a adaptive-apps
rad resource list -a adaptive-apps
kubectl get pods -n trading
```

Expose the frontend and verify the app works:

```bash
rad resource expose Applications.Core/containers frontend \
    -a adaptive-apps \
    --port 3000 \
    --remote-port 3000
```

Open `http://localhost:3000` and sign in with the `authUsername` and `authPassword` values used during deployment.

#### What to discuss

- *"Which command named the environment?"* (`--environment env-local-prod`; the app model did not hard-code the target.)
- *"Where did the database host, MQTT host, and optional AI endpoint come from?"* (Recipe outputs projected through Radius connections and explicit secret wiring.)
- *"What would you expect to change when the same app goes to Azure?"* (The recipes and provider scope, not the app resource declarations.)

---

### Stage 3 - Confirm or prepare the second environment

The second environment should already have a Kubernetes cluster and Radius control plane from Challenge 1 and Challenge 2. The Azure-backed example below uses the AKS workspace from Challenge 2 and the repository's `radius/aks-env.bicep` from Challenge 4.

Switch to the second workspace and verify the group exists:

```bash
rad workspace switch <second-workspace>
rad group list
```

If `rg-trading` is not listed, create it. Then switch to it:

```bash
rad group create rg-trading
rad group switch rg-trading
```

Register Azure credentials with the Radius control plane if the team has not already done so. If the team followed the AKS preparation guide and used workload identity for Radius, prefer the `rad credential register azure wi ...` pattern from Challenge 2. If they are using a client secret for the hack environment, this form also works:

```bash
rad credential register azure \
    --client-id <appId> \
    --client-secret <password> \
    --tenant-id <tenant>
```

Deploy the Azure-backed environment definition:

```bash
rad deploy radius/aks-env.bicep \
    --group rg-trading \
    --environment env-azure-prod \
    --parameters namespace=env-azure-prod \
    --parameters environmentName=env-azure-prod \
    --parameters azureSubscriptionId=<subscription-id> \
    --parameters azureResourceGroup=<resource-group>
```

Verify the recipe mapping:

```bash
rad environment show env-azure-prod
rad recipe list --environment env-azure-prod
```

The same resource types should now point to Azure-backed recipes where appropriate:

| Resource type | AKS / Azure recipe outcome |
|---|---|
| `Radius.Resources/postgreSqlDatabases` | PostgreSQL container recipe (`postgres:latest`) in the current repo; replaceable by a managed PostgreSQL recipe later |
| `Radius.Resources/mqttBrokers` | Azure Event Grid MQTT namespace endpoint |
| `Radius.Resources/workloadIdentities` | Pass-through of pre-provisioned Azure workload identity values |
| `Radius.Resources/aiModels` | Azure OpenAI account and deployment when AI is enabled |
| `Radius.Resources/governance` | OPA policy decision point in Kubernetes |
| `Radius.Resources/agentGuardrails` | Agent Governance Toolkit sidecar support when guardrails are enabled |

`Radius.Resources/idProviders` is intentionally not Azure-backed in `aks-env.bicep`; teams will replace identity-provider behavior in a later challenge.

> **Known limitation:** The Azure Event Grid MQTT recipe provisions the namespace endpoint and returns the connection shape expected by `Radius.Resources/mqttBrokers`. Event Grid MQTT still requires authenticated clients plus topic-space and permission-binding configuration before publish/subscribe flows will work end to end. Treat this as a portability demonstration unless the team has also completed the Event Grid MQTT identity and permission setup.

#### Coaching tip

If a team asks why the Azure environment uses a different Bicep file, point out that `aks-env.bicep` is platform-team code. It defines the target platform, provider scope, and recipe catalog. The portability test is whether `radius/app.bicep` stays unchanged.

---

### Stage 4 - Deploy the same app to the second environment

Run the same application file against the second workspace and environment. The file path remains `radius/app.bicep`.

For the AKS/Azure example, pass the managed identity values created for the backend and frontend workloads. The repository includes a helper at `tutorials/getting-started/assets/app-wi-setup.sh`; the important output for this challenge is the managed identity client ID for each workload. AKS must have workload identity enabled, and the identities still need the Event Grid MQTT permissions required by the target namespace.

```bash
rad deploy radius/app.bicep \
    --group rg-trading \
    --environment env-azure-prod \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=<your-password> \
    --parameters backendClientId=<backend-managed-identity-client-id> \
    --parameters frontendClientId=<frontend-managed-identity-client-id> \
    --parameters workloadIdentityTenantId=<tenant-id>
```

For a second environment that still uses the Kubernetes MQTT recipe, the workload identity parameters can remain omitted:

```bash
rad deploy radius/app.bicep \
    --group rg-trading \
    --environment env-azure-prod \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=<your-password>
```

For AI-enabled Azure deployments, keep `aiProvider=local`. In this application model, `local` means "use the environment-registered `Radius.Resources/aiModels` recipe." In the Azure environment, that recipe is `ai-agent-azure-openai:latest`, so the backing implementation is Azure OpenAI:

```bash
rad deploy radius/app.bicep \
    --group rg-trading \
    --environment env-azure-prod \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=<your-password> \
    --parameters backendClientId=<backend-managed-identity-client-id> \
    --parameters frontendClientId=<frontend-managed-identity-client-id> \
    --parameters workloadIdentityTenantId=<tenant-id> \
    --parameters aiProvider=local \
    --parameters aiModel=gpt-4o
```

Validate the second deployment:

```bash
rad app graph -a adaptive-apps
rad resource list -a adaptive-apps
kubectl get pods -n env-azure-prod
```

If the deployment is on AKS, also verify the Azure resources were created in the configured Azure resource group:

```bash
az resource list \
    --resource-group <resource-group> \
    --output table
```

Expose the frontend from the second environment:

```bash
rad resource expose Applications.Core/containers frontend \
    -a adaptive-apps \
    --port 3000 \
    --remote-port 3000
```

Open `http://localhost:3000` and verify the deployment is reachable. If the second environment uses Azure Event Grid MQTT without topic spaces, permission bindings, and client authorization configured, browser access can work while MQTT-backed flows fail; use that as a coaching moment about which portability gaps belong in recipes versus separate platform setup.

### Stage 5 - Compare the two deployments

Have teams compare the application graph and the backing platform resources in both environments.

In the first workspace:

```bash
rad workspace switch <first-workspace>
rad app graph -a adaptive-apps
rad resource list -a adaptive-apps
rad recipe list --environment env-local-prod
```

In the second workspace:

```bash
rad workspace switch <second-workspace>
rad app graph -a adaptive-apps
rad resource list -a adaptive-apps
rad recipe list --environment env-azure-prod
```

The app graph should look familiar because the Radius application resources are the same. The recipe outputs and backing infrastructure should differ because the environments are different.

#### Expected differences

| Area | First environment | Second environment |
|---|---|---|
| `radius/app.bicep` | Same file | Same file |
| Kubernetes namespace | Usually `env-local-prod` | Usually `env-azure-prod` on the second cluster |
| Database backend | Containerized PostgreSQL | Containerized PostgreSQL in the current repo; can be swapped by changing the environment recipe |
| MQTT backend | Mosquitto | Azure Event Grid MQTT endpoint; full messaging also needs topic-space and permission setup |
| Workload identity | Local/no-op service account outputs | Pre-provisioned Azure workload identity outputs |
| AI backend, if enabled | Kaito / local OpenAI-compatible endpoint | Azure OpenAI through the recipe |

#### Coaching questions

- *"What changed between the two app deployments?"* (Workspace, environment, and parameter values. The app model stayed the same.)
- *"Where is the platform-specific logic located?"* (In the environment's recipe registrations and the recipe Bicep files.)
- *"Why does the app still connect to the database without a new connection string?"* (The resource type contract and recipe outputs are stable; Radius projects the outputs into the container through connections.)
- *"What would make this app less portable?"* (Hard-coded hostnames, cloud-specific SDK setup in `app.bicep`, direct Azure resource declarations in the application model, or environment-specific image builds.)
- *"Who owns each file now?"* (`radius/app.bicep` is app-team code; `radius/*-env.bicep` and `radius/recipes/*` are platform-team code.)

## Validation

At the end of this challenge, teams should be able to demonstrate:

- The same `radius/app.bicep` file was used for both deployments.
- Each environment has a recipe list for the same portable resource types.
- The application deploys and is reachable in both environments; full runtime parity on the Azure MQTT path requires the Event Grid topic-space and permission setup called out above.
- The backing infrastructure can differ by environment without changing the application resource declarations; in the current repo, MQTT, workload identity, and AI show the clearest differences.
- The team can explain which changes were environment/platform changes and which were application deployment parameters.

## Common Issues

**Recipes are missing in the second environment**

Run `rad recipe list --environment env-azure-prod` in the second workspace. If the list is empty or only partially populated, redeploy the correct environment Bicep (`radius/local-env.bicep` or `radius/aks-env.bicep`) before deploying the app.

**The app deployed to the wrong cluster**

Run `rad workspace list` and `kubectl config current-context`. The `rad` workspace chooses the Radius control plane; the Kubernetes context is used when creating or updating that workspace. Teams often switch `kubectl` context but forget to switch `rad` workspace.

**Azure recipe deployment fails with authorization errors**

Confirm `rad credential register azure` was run against the second workspace and that the service principal has permission on the Azure resource group passed to `aks-env.bicep`.

**Azure MQTT auth or publish/subscribe fails**

Confirm AKS workload identity is enabled, the backend and frontend managed identity client IDs were passed to `app.bicep`, and the Event Grid namespace has the topic-space and permission-binding configuration required for those identities. The current `mqtt-azure-event-grid` recipe creates the namespace endpoint; it does not complete all MQTT authorization setup.

**The second deployment overwrote the first**

If both deployments target the same Radius control plane and group, the fixed application name `adaptive-apps` represents the same Radius application resource. Use separate workspaces/control planes for the cleanest environment split, or separate Radius groups within `rg-trading` if the team is simulating multiple environments on one control plane.

**AI works locally but fails in Azure**

Check that the Azure environment recipe for `Radius.Resources/aiModels` is registered and that the requested `aiModel` value is available in the target Azure OpenAI region. The app still passes `aiProvider=local` because that tells `app.bicep` to use the recipe-backed `aiModels` resource; the environment decides whether the recipe produces Kaito or Azure OpenAI.
