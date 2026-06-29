# Challenge 05 - Port the App Across Environments - Coach's Guide

[< Previous Solution](./Solution-04.md) - **[Home](./README.md)** - [Next Solution >](./Solution-06.md)

## Notes & Guidance

- Challenge 05 is the direct continuation of Challenge 04. Teams have just learned that a recipe is the implementation behind a Radius resource type. Now they must prove that the application can move because that implementation lives in the environment, not in the application model.
- Anchor the challenge in the Challenge 04 handoff: teams should already understand the `context` object, the required `result` output, and the database recipe registered for `Radius.Resources/postgreSqlDatabases`. In Challenge 04 teams also practiced *authoring* a SQL Server recipe for `Radius.Resources/sqlDatabases` — that dashboard exercise teaches the recipe pattern, while the shipped application actually deploys `postgreSqlDatabases`.
- Expected time: **45-75 minutes** if both environments were prepared in earlier challenges. Add time if a second cluster, Azure credentials, or the Azure resource group still need to be provisioned.
- Biggest coaching risk: teams try to make the second deployment work by editing the application model. Redirect them to the environment, recipe registration, workspace, group, and parameter layer.
- Treat the database resource as the continuity proof from Challenge 04. The application asks for `Radius.Resources/postgreSqlDatabases`; the environment chooses the recipe implementation (a PostgreSQL container locally, Azure Database for PostgreSQL on AKS).
- Be explicit about the boundary between a portability demonstration and full runtime parity. Azure Event Grid MQTT returns a namespace endpoint, but end-to-end publish/subscribe also needs clients, topic spaces, and permission bindings.

## Key Concepts - Portability After Recipes

### The Challenge 04 handoff

Challenge 04 taught teams to place implementation behind a resource type. The important carry-forward is not the specific command they ran; it is the layer boundary they established:

```
Application model                      Environment / recipe layer
----------------------------------     ---------------------------------------
Radius.Resources/postgreSqlDatabases   database recipe registered by env
  input: size, environment             reads context.resource.properties.size
  output: host, port, database         returns result.values.*
  secret: password                     returns result.secrets.password
```

Challenge 05 should start from this state. Teams should not rediscover what recipes are; they should use recipes to prove the application model can be deployed somewhere else.

### New concepts to coach in this challenge

Challenge 05 does not introduce a brand-new Radius primitive, but it combines earlier primitives in ways teams may not have reasoned about yet:

| Concept | What coaches should make explicit |
|---|---|
| Portability boundary | The application model is the invariant; environment files, recipe registrations, and selected parameters are the allowed variation. |
| Deployment target | Workspace, Radius group, environment, and Kubernetes context together decide where the app lands. Teams should know all four before they run `rad deploy`. |
| Recipe readiness | The second environment is not ready just because Radius exists there. It must have recipes for the resource types the app asks for. |
| Deployment portability vs. runtime parity | A deployment can demonstrate portability even if a backing service such as Event Grid MQTT still needs extra platform setup for full behavior. |
| Evidence of portability | Teams need proof: same app model, comparable app graph, registered recipes, reachable frontend, and a clear list of changed parameters. |
| Ownership split | App teams own the application model and parameter choices; platform teams own environments, provider scope, and recipes. |

### What stays the same and what changes

| Layer | Should stay the same? | Coach emphasis |
|---|---:|---|
| Application model | Yes | The same file and resource declarations are deployed to both environments. |
| Resource type contracts | Yes | `Radius.Resources/postgreSqlDatabases`, `mqttBrokers`, `workloadIdentities`, and optional `aiModels` remain the application-facing shape. |
| Environment and recipe registrations | Can differ | Platform teams choose which recipe satisfies each capability in each environment. |
| Deployment target | Differs | Workspace, Radius group, environment, and sometimes Kubernetes context change deliberately. |
| Parameter values | Can differ | Identity client IDs, tenant IDs, image tags, and optional AI model values may differ by environment. |

### The database capability as the continuity thread

The database recipe from Challenge 04 is the easiest way to show continuity. In Challenge 04 teams also practiced authoring a SQL Server recipe for `Radius.Resources/sqlDatabases` in the dashboard — use that as the mental model for *how* a recipe implements a type — but the shipped application deploys `Radius.Resources/postgreSqlDatabases`, which each environment satisfies with its own recipe. Ask teams:

- "Where is the database implementation expressed?"
- "What would have to change to use a different database implementation?"
- "What did the application model need to know about the database server name, resource group, or SKU?"

The answer should keep pointing back to the same principle: the application asks for a database capability; the environment and recipe own the implementation.

### Targeting discipline

The cleanest demo uses the workspaces established in Challenge 2: `ws-local-prod` for `env-local-prod` and `ws-azure-prod` for `env-azure-prod`. A one-cluster fallback can still demonstrate the pattern with two Radius environments or groups, but coaches should make teams name that limitation. The important teaching point is that `rad` workspace/group/environment and `kubectl` context are separate sources of truth and can drift.

### Known portability gaps to discuss

- Azure Event Grid MQTT recipe output is not the same as full MQTT runtime parity. The namespace endpoint alone does not configure clients, topic spaces, or permission bindings.
- Workload identity values are expected in Azure; do not weaken identity just to get a deployment green.
- Optional AI can demonstrate the same recipe seam: the application selects the recipe-backed AI capability, while the environment decides whether the backing service is local/Kaito or Azure OpenAI.

---

## Solution Guide

### Stage 1 - Confirm the Challenge 04 end state

Start by confirming that the team is really continuing from Challenge 04 rather than rebuilding it. They should be able to show the database recipe registration (`Radius.Resources/postgreSqlDatabases`) and explain the `context` and `result` contract. The example below is based on the "local" environment, replace when needed.

```bash
rad workspace switch ws-local-prod
rad group switch rg-trading

rad environment show env-local-prod
rad recipe list --environment env-local-prod
```

The exact names may differ if the team used the AKS preparation sample path (`aks-trading` / `trading`) or another agreed naming convention, but they should not be unknown at this point. The important checks are:

| Check | Expected outcome |
|---|---|
| Database recipe | `Radius.Resources/postgreSqlDatabases` is registered (Challenge 04 Stage 2 deployed `local-env.bicep` / `aks-env.bicep`, which register it). |
| MQTT recipe | `Radius.Resources/mqttBrokers` is registered for the first environment. |
| Workload identity recipe | `Radius.Resources/workloadIdentities` is registered or intentionally handled for the first environment. |
| Optional AI recipe | `Radius.Resources/aiModels` is registered if the team will test AI portability. |

If `Radius.Resources/postgreSqlDatabases` is missing, send the team back to Challenge 04. Challenge 05 depends on the environment having the recipe mapping in place.

#### What to discuss

- *"Which recipe registration came from Challenge 04?"*
- *"What values does the database recipe return through `result.values` and `result.secrets`?"*
- *"What information did the application model not need to know about the database server?"*

---

### Parity checklist for both environments

Use the same teaching sequence in each environment, with only environment-specific values changing:

1. Target workspace/group/environment.
2. Set or verify credentials and RBAC (only where needed).
3. Verify required recipes for required resource types.
4. Deploy the same application model with environment-specific parameters.
5. Validate graph, resources, and runtime reachability.

### Stage 2 - Deploy the app to the first environment (ws-local-prod)

Use the parity checklist explicitly for the environment ws-local-prod, for ws-azure-prod go to the second environment,

#### Step 1 - Target workspace/group/environment (first environment)

```bash
rad workspace switch ws-local-prod
rad group switch rg-trading
rad environment show env-local-prod
```

#### Step 2 - Set/verify credentials and RBAC (only where needed)

For this local-first deployment path, no additional Azure credential or RBAC setup is required before deployment.

#### Step 3 - Verify required recipes for required resource types

```bash
rad recipe list --environment env-local-prod
```

First make sure the rad resource types are bundled:

**Bash:**

```bash
cd radius
rad bicep publish-extension --from-file radius/resource-types/types.yaml --target types.tgz
```

**PowerShell:**

```powershell
cd radius
rad bicep publish-extension --from-file resource-types/types.yaml --target types.tgz
```

Use the same application file the team will later deploy elsewhere. The command below uses the sample group and environment names; substitute the team's names if Challenge 04 used different ones.

#### Step 4 - Deploy the same app model with first-environment parameters

**Bash:**

```bash
rad deploy radius/app.bicep \
    --group rg-trading \
    --environment env-local-prod \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=<your-password>
```

**PowerShell:**

```powershell
rad deploy radius/app.bicep `
    --group rg-trading `
    --environment env-local-prod `
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps `
    --parameters imageTag=latest `
    --parameters authUsername=admin `
    --parameters authPassword=<your-password>
```

If the team is validating AI portability, enable the recipe-backed AI resource:

**Bash:**

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

**PowerShell:**

```powershell
rad deploy radius/app.bicep `
    --group rg-trading `
    --environment env-local-prod `
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps `
    --parameters imageTag=latest `
    --parameters authUsername=admin `
    --parameters authPassword=<your-password> `
    --parameters aiProvider=local `
    --parameters aiModel=qwen2.5-coder-7b-instruct
```

#### Step 5 - Validate graph/resources/runtime

Validate the app model and backing resources:

```bash
rad app graph -a adaptive-apps
rad resource list -a adaptive-apps
kubectl get pods -n env-local-prod
```

Expose the frontend and verify the app is reachable:

**Bash:**

```bash
rad resource expose Applications.Core/containers frontend \
    -a adaptive-apps \
    --port 3000 \
    --remote-port 3000
```

**PowerShell:**

```powershell
rad resource expose Applications.Core/containers frontend `
    -a adaptive-apps `
    --port 3000 `
    --remote-port 3000
```

Open `http://localhost:3000` and sign in with the `authUsername` and `authPassword` values used during deployment.

#### What to discuss

- *"Which command named the environment?"*
- *"Where did the database host and password come from?"*
- *"What would you expect to change when the same app goes to a second environment?"*

---

### Stage 3 - Confirm or prepare the second environment (ws-azure-prod)

Apply the same parity checklist for the second environment (ws-azure-prod). This stage covers Steps 1-3; Stage 4 continues with Steps 4-5.

The second environment should already have a Kubernetes cluster and Radius control plane from Challenges 1 and 2. Challenge 4 should have given teams enough recipe knowledge to register the same required resource types in this environment.

#### Step 1 - Target workspace/group/environment (second environment)

Switch to the second workspace established in Challenge 2 and verify the Radius group exists:

```bash
rad workspace switch ws-azure-prod
rad group list
```

If `rg-trading` is not listed, create it. Then switch to it:

```bash
rad group create rg-trading
rad group switch rg-trading
```

```bash
rad environment show env-azure-prod
```

#### Step 2 - Set/verify credentials and RBAC (only where needed)

Register Azure credentials with the Radius control plane if the environment's recipes need Azure provider scope and credentials. The client ID can be found in the console output from earlier commands or by searching for {AKS_CLUSTER}-radius-app.

Use the command variant that matches your identity model and Radius CLI version:

**Bash:**

```bash
export RADIUS_APP_ID=<appId>
export TENANTID=<tenantId>
```

```bash
# Workload identity (recommended for AKS). No client secret required.
rad credential register azure wi \
    --client-id $APP_ID \
    --tenant-id TENANTID

# Service principal (use only when workload identity is unavailable).
rad credential register azure sp \
    --client-id $APP_ID \
    --client-secret <password> \
    --tenant-id $TENANTID
```

**PowerShell:**

```powershell
# Workload identity (recommended for AKS). No client secret required.
rad credential register azure wi `
    --client-id $APP_ID `
    --tenant-id $TENANTID

# Service principal (use only when workload identity is unavailable).
rad credential register azure sp `
    --client-id $APP_ID `
    --client-secret <password> `
    --tenant-id $TENANTID
```

Verify registration:

```bash
rad credential show azure
```

Grant the Radius identity permission on the Azure resource group used by `env-azure-prod`.
This is required for recipes such as `mqtt-azure-event-grid` to create Azure resources.

**Bash:**

```bash
export AZURE_SUBSCRIPTION=<subscription-id>
export RESOURCE_GROUP=<azure-resource-group>

# Use the same appId passed to `rad credential register azure wi --client-id ...`
export RADIUS_APP_ID=<radius-workload-identity-appId>
export RADIUS_SP_OBJECT_ID=$(az ad sp show --id "$RADIUS_APP_ID" --query id -o tsv)

# Prevent MissingSubscription by setting and passing subscription explicitly.
az account set --subscription "$AZURE_SUBSCRIPTION"

az role assignment create \
    --subscription "$AZURE_SUBSCRIPTION" \
    --assignee-object-id "$RADIUS_SP_OBJECT_ID" \
    --assignee-principal-type ServicePrincipal \
    --role Contributor \
    --scope "/subscriptions/$AZURE_SUBSCRIPTION/resourceGroups/$RESOURCE_GROUP"

# Verify
az role assignment list \
    --subscription "$AZURE_SUBSCRIPTION" \
    --assignee-object-id "$RADIUS_SP_OBJECT_ID" \
    --scope "/subscriptions/$AZURE_SUBSCRIPTION/resourceGroups/$RESOURCE_GROUP" \
    -o table
```

**PowerShell:**

```powershell
$AZURE_SUBSCRIPTION = "<subscription-id>"
$RESOURCE_GROUP = "<azure-resource-group>"

# Use the same appId passed to `rad credential register azure wi --client-id ...`
$RADIUS_APP_ID = "<radius-workload-identity-appId>"
$RADIUS_SP_OBJECT_ID = az ad sp show --id "$RADIUS_APP_ID" --query id -o tsv

# Prevent MissingSubscription by setting and passing subscription explicitly.
az account set --subscription "$AZURE_SUBSCRIPTION"

az role assignment create `
    --subscription "$AZURE_SUBSCRIPTION" `
    --assignee-object-id "$RADIUS_SP_OBJECT_ID" `
    --assignee-principal-type ServicePrincipal `
    --role Contributor `
    --scope "/subscriptions/$AZURE_SUBSCRIPTION/resourceGroups/$RESOURCE_GROUP"

# Verify
az role assignment list `
    --subscription "$AZURE_SUBSCRIPTION" `
    --assignee-object-id "$RADIUS_SP_OBJECT_ID" `
    --scope "/subscriptions/$AZURE_SUBSCRIPTION/resourceGroups/$RESOURCE_GROUP" `
    -o table
```

If teams see `(MissingSubscription) The request did not have a subscription...`, the Azure CLI context is not set for the target subscription or `--subscription` was omitted.

If teams see `Error: unknown flag: --client-id`, they likely ran `rad credential register azure` without `wi` or `sp`. In current CLI versions, `--client-id` is valid only under `azure wi` or `azure sp`.

#### Step 3 - Verify required recipes for required resource types

Ensure the second environment has equivalent recipe mappings. The database capability should still be `Radius.Resources/postgreSqlDatabases`; the recipe implementation differs per environment — a PostgreSQL container locally and Azure Database for PostgreSQL Flexible Server on AKS.

```bash
rad environment show env-azure-prod
rad recipe list --environment env-azure-prod
```

Expected recipe readiness:

| Resource type | Second environment expectation |
|---|---|
| `Radius.Resources/postgreSqlDatabases` | Database recipe registered for the second environment (Azure Database for PostgreSQL Flexible Server on AKS). |
| `Radius.Resources/mqttBrokers` | Azure Event Grid MQTT or another broker recipe registered by the platform team. |
| `Radius.Resources/workloadIdentities` | Azure workload identity recipe or equivalent identity mapping. |
| `Radius.Resources/aiModels` | Azure OpenAI recipe if AI portability is in scope. |
| `Radius.Resources/governance` | Governance recipe if the app deployment enables governance. |
| `Radius.Resources/agentGuardrails` | Guardrails recipe if the app deployment enables guardrails. |

#### Coaching tip

If a team asks why the second environment can use a different recipe or environment file, point out that environment and recipe files are platform-team code. The portability test is whether the application model stays unchanged.

---

### Stage 4 - Continue parity checklist on second environment (deploy + validate)

This stage completes Step 4 and Step 5 for the second environment.

#### Step 4 - Deploy the same app model with second-environment parameters

Run the same application file against the second workspace and environment. The file path remains `radius/app.bicep`.

For the AKS/Azure example, pass the managed identity values created for the backend and frontend workloads. AKS must have workload identity enabled, and the identities still need the Event Grid MQTT permissions required by the target namespace.

#### Get the workload identity parameter values

The `backendClientId` and `frontendClientId` values come from the user-assigned managed identities that are federated to the app's Kubernetes service accounts. The repository helper script creates those identities and prints the exact `--parameters <workload>ClientId=...` value to use.

First get the AKS OIDC issuer URL and tenant ID:

**Bash:**

If you have completed the previous solutions these are stored in variables, check if populated:
```bash
echo $AKS_OIDC_ISSUER
echo $TENANT_ID
```

If exist skip the following step:

```bash
export AKS_OIDC_ISSUER=$(az aks show \
    --name <aks-cluster-name> \
    --resource-group <aks-resource-group> \
    --query oidcIssuerProfile.issuerUrl \
    --output tsv)

export TENANT_ID=$(az account show --query tenantId --output tsv)
```

**PowerShell:**

If you have completed the previous solutions these are stored in variables, check if populated:
```powershell
echo $env:AKS_OIDC_ISSUER
echo $env:TENANT_ID
```

If exist skip the following step:

```powershell
$AKS_OIDC_ISSUER = az aks show `
    --name <aks-cluster-name> `
    --resource-group <aks-resource-group> `
    --query oidcIssuerProfile.issuerUrl `
    --output tsv

$TENANT_ID = az account show --query tenantId --output tsv
```

The helper federates a managed identity to a Kubernetes service account in a specific namespace, so the namespace argument must match the namespace Radius actually deploys the app into. That namespace is the target environment's `compute.namespace`, which is not necessarily the same string as the environment name. Confirm it first:

**Bash:**

```bash
rad environment show env-azure-prod --output json | jq -r '.properties.compute.namespace'
```

**PowerShell:**

```powershell
(rad environment show env-azure-prod --output json | ConvertFrom-Json).properties.compute.namespace
```

With the shipped `aks-env.bicep` defaults this namespace is `trading` (it matches the environment name in that sample); the Challenge 2 teaching path configures it as `env-azure-prod`. Use whatever your team configured for `<environment-namespace>` below. The service account name is the value the application model binds to — `default` unless the team changed `workloadIdentityServiceAccountName`.

Then run the helper once for each workload:

**Bash:**

```bash
./tutorials/getting-started/assets/app-wi-setup.sh \
    backend \
    $RESOURCE_GROUP \
    $AZURE_SUBSCRIPTION \
    "$AKS_OIDC_ISSUER" \
    <environment-namespace> \
    default

./tutorials/getting-started/assets/app-wi-setup.sh \
    frontend \
    $RESOURCE_GROUP \
    $AZURE_SUBSCRIPTION \
    "$AKS_OIDC_ISSUER" \
    <environment-namespace> \
    default
```

**PowerShell:**

```powershell
& ./tutorials/getting-started/assets/app-wi-setup.sh `
    backend `
    $RESOURCE_GROUP `
    $AZURE_SUBSCRIPTION `
    "$AKS_OIDC_ISSUER" `
    <environment-namespace> `
    default

& ./tutorials/getting-started/assets/app-wi-setup.sh `
    frontend `
    $RESOURCE_GROUP `
    $AZURE_SUBSCRIPTION `
    "$AKS_OIDC_ISSUER" `
    <environment-namespace> `
    default
```

The helper creates only the managed identity and its federated credential. Granting those identities the Event Grid (or other backing-service) permissions the workloads need is separate platform setup, and is one reason a green deployment is not yet full runtime parity.

Copy the `Client ID` from each script run:

| Deployment parameter | Source |
|---|---|
| `backendClientId` | `Client ID` printed by the `backend` script run |
| `frontendClientId` | `Client ID` printed by the `frontend` script run |
| `workloadIdentityTenantId` | `$TENANT_ID` from `az account show` |

For example:

**Bash:**

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

**PowerShell:**

```powershell
rad deploy radius/app.bicep `
    --group rg-trading `
    --environment env-azure-prod `
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps `
    --parameters imageTag=latest `
    --parameters authUsername=admin `
    --parameters authPassword=<your-password> `
    --parameters backendClientId=<backend-managed-identity-client-id> `
    --parameters frontendClientId=<frontend-managed-identity-client-id> `
    --parameters workloadIdentityTenantId=<tenant-id>
```

For a second environment that does not require Azure workload identity parameters, omit those values:

**Bash:**

```bash
rad deploy radius/app.bicep \
    --group rg-trading \
    --environment env-azure-prod \
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
    --parameters imageTag=latest \
    --parameters authUsername=admin \
    --parameters authPassword=<your-password>
```

**PowerShell:**

```powershell
rad deploy radius/app.bicep `
    --group rg-trading `
    --environment env-azure-prod `
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps `
    --parameters imageTag=latest `
    --parameters authUsername=admin `
    --parameters authPassword=<your-password>
```

For AI-enabled Azure deployments, keep `aiProvider=local` if the application model uses that value to select the recipe-backed AI resource. The environment decides whether the recipe produces a local/Kaito endpoint or Azure OpenAI:

**Bash:**

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

**PowerShell:**

```powershell
rad deploy radius/app.bicep `
    --group rg-trading `
    --environment env-azure-prod `
    --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps `
    --parameters imageTag=latest `
    --parameters authUsername=admin `
    --parameters authPassword=<your-password> `
    --parameters backendClientId=<backend-managed-identity-client-id> `
    --parameters frontendClientId=<frontend-managed-identity-client-id> `
    --parameters workloadIdentityTenantId=<tenant-id> `
    --parameters aiProvider=local `
    --parameters aiModel=gpt-4o
```

#### Step 5 - Validate graph/resources/runtime

Validate the second deployment:

```bash
rad app graph -a adaptive-apps
rad resource list -a adaptive-apps
kubectl get pods -n <environment-namespace>   # e.g. trading (aks-env.bicep default) or env-azure-prod (Challenge 2 path)
```

If the deployment is on AKS, also verify the Azure resources were created in the configured Azure resource group:

```bash
az resource list \
    --subscription <subscription-id> \
    --resource-group <resource-group> \
    --output table
```

Expose the frontend from the second environment. If the first expose session is still running on local port 3000, stop it first or pick a different local port:

**Bash:**

```bash
rad resource expose Applications.Core/containers frontend \
    -a adaptive-apps \
    --port 3001 \
    --remote-port 3000
```

**PowerShell:**

```powershell
rad resource expose Applications.Core/containers frontend `
    -a adaptive-apps `
    --port 3001 `
    --remote-port 3000
```

Open `http://localhost:3001` and verify the deployment is reachable. If the second environment uses Azure Event Grid MQTT without topic spaces, permission bindings, and client authorization configured, browser access can work while MQTT-backed flows fail; use that as a coaching moment about which portability gaps belong in recipes versus separate platform setup.

---

### Stage 5 - Compare the two deployments

Have teams compare the application graph and the backing platform resources in both environments.

**First workspace:**

```bash
rad workspace switch ws-local-prod
rad app graph -a adaptive-apps
rad resource list -a adaptive-apps
rad recipe list --environment env-local-prod
```

**Second workspace:**

```bash
rad workspace switch ws-azure-prod
rad app graph -a adaptive-apps
rad resource list -a adaptive-apps
rad recipe list --environment env-azure-prod
```

The app graph should look familiar because the Radius application resources are the same. The recipe outputs and backing infrastructure should differ because the environments are different.

#### Expected differences

| Area | First environment | Second environment |
|---|---|---|
| Application model | Same file | Same file |
| Kubernetes namespace | Usually `env-local-prod` | `trading` with `aks-env.bicep` defaults, or `env-azure-prod` on the Challenge 2 path |
| Database backend | PostgreSQL container recipe (`postgres:latest`) | Azure Database for PostgreSQL Flexible Server recipe (`postgres-azure-flex:latest`, AVM) |
| MQTT backend | Local broker or first-environment recipe | Azure Event Grid MQTT endpoint or second-environment broker recipe |
| Workload identity | Local/no-op or first-environment identity recipe | Azure workload identity values |
| AI backend, if enabled | Kaito / local OpenAI-compatible endpoint | Azure OpenAI through the recipe |

#### Coaching questions

- *"What changed between the two app deployments?"* (Workspace, environment, and parameter values. The app model stayed the same.)
- *"Where is the platform-specific logic located?"* (In the environment's recipe registrations and the recipe Bicep files.)
- *"Why does the app still connect to the database without a new hard-coded connection string?"* (The resource type contract and recipe outputs are stable; Radius projects the outputs into the container through connections.)
- *"What would make this app less portable?"* (Hard-coded hostnames, cloud-specific SDK setup in the application model, direct Azure resource declarations in the application model, or environment-specific image builds.)
- *"Who owns each file now?"* (The application model is app-team code; environment and recipe files are platform-team code.)

### Validation checklist

At the end of this challenge, teams should be able to demonstrate:

- The same application model was used for both deployments.
- Each environment registers recipes for the resource types the deployed app actually uses, including `Radius.Resources/postgreSqlDatabases`.
- The application deploys and is reachable in both environments; full runtime parity on the Azure MQTT path requires the Event Grid topic-space and permission setup called out above.
- The backing infrastructure can differ by environment without changing the application resource declarations.
- The team can explain which changes were environment/platform changes and which were application deployment parameters.

### Common Issues

**Deployment fails with "RecipeNotFoundFailure: could not find recipe 'default'"**

**Symptom:**
```
Error: {
  "code": "ResourceDeploymentFailure",
  "message": "Failed",
  "details": [{
    "code": "RecipeNotFoundFailure",
    "message": "could not find recipe \"default\" in environment \"/planes/radius/local/resourcegroups/rg-trading/providers/Applications.Core/environments/env-local-prod\""
  }]
}
```

This error occurs for `Radius.Resources/workloadIdentities`, `Radius.Resources/mqttBrokers`, `Radius.Resources/postgreSqlDatabases`, or other resource types.

**Root cause:** Challenge 04 Stage 2 has not been completed. The environment exists, but no recipes have been registered for the resource types the application is trying to deploy.

**Solution:** Return to Challenge 04 and complete **Stage 2 — Register the pre-built recipes**. Run:

**Bash:**

```bash
rad deploy radius/local-env.bicep --group rg-trading --environment env-local-prod
```

**PowerShell:**

```powershell
rad deploy radius/local-env.bicep --group rg-trading --environment env-local-prod
```

This registers all required recipes (`postgreSqlDatabases`, `mqttBrokers`, `workloadIdentities`, etc.) in the environment. Then retry `rad deploy radius/app.bicep ...`.

**The second deployment overwrote the first**

If both deployments target the same Radius control plane and group, the fixed application name `adaptive-apps` represents the same Radius application resource. Use separate workspaces/control planes for the cleanest environment split, or a separate Radius group per environment if the team is simulating multiple environments on one control plane.

**The app deployed to the wrong cluster**

```bash
rad workspace list
kubectl config current-context
```

The `rad` workspace chooses the Radius control plane; the Kubernetes context is used when creating or updating that workspace. Teams often switch `kubectl` context but forget to switch `rad` workspace.

**The database recipe is missing**

```bash
rad recipe list --environment <environment-name>
```

If `Radius.Resources/postgreSqlDatabases` is missing, return to Challenge 04 and deploy `local-env.bicep` / `aks-env.bicep` (Stage 2) to register the database recipe before deploying the app.

**Azure recipe deployment fails with authorization errors**

Confirm `rad credential register azure` was run against the second workspace and that the configured identity has permission on the Azure resource group used by the second environment.

If no RBAC assignment exists yet, run:

**Bash:**

```bash
export AZURE_SUBSCRIPTION=<subscription-id>
export RESOURCE_GROUP=<azure-resource-group>
export RADIUS_APP_ID=<radius-workload-identity-appId>
export RADIUS_SP_OBJECT_ID=$(az ad sp show --id "$RADIUS_APP_ID" --query id -o tsv)

az account set --subscription "$AZURE_SUBSCRIPTION"

az role assignment create \
    --subscription "$AZURE_SUBSCRIPTION" \
    --assignee-object-id "$RADIUS_SP_OBJECT_ID" \
    --assignee-principal-type ServicePrincipal \
    --role Contributor \
    --scope "/subscriptions/$AZURE_SUBSCRIPTION/resourceGroups/$RESOURCE_GROUP"
```

**PowerShell:**

```powershell
$AZURE_SUBSCRIPTION = "<subscription-id>"
$RESOURCE_GROUP = "<azure-resource-group>"
$RADIUS_APP_ID = "<radius-workload-identity-appId>"
$RADIUS_SP_OBJECT_ID = az ad sp show --id "$RADIUS_APP_ID" --query id -o tsv

az account set --subscription "$AZURE_SUBSCRIPTION"

az role assignment create `
    --subscription "$AZURE_SUBSCRIPTION" `
    --assignee-object-id "$RADIUS_SP_OBJECT_ID" `
    --assignee-principal-type ServicePrincipal `
    --role Contributor `
    --scope "/subscriptions/$AZURE_SUBSCRIPTION/resourceGroups/$RESOURCE_GROUP"
```

Then retry `rad deploy ...`.

**Azure MQTT auth or publish/subscribe fails**

Confirm AKS workload identity is enabled, the backend and frontend managed identity client IDs were passed to the application deployment, and the Event Grid namespace has the topic-space and permission-binding configuration required for those identities. The MQTT recipe may create the namespace endpoint without completing all MQTT authorization setup.

**AI works locally but fails in Azure**

Check that the Azure environment recipe for `Radius.Resources/aiModels` is registered and that the requested `aiModel` value is available in the target Azure OpenAI region. The app can still use a recipe-selection parameter while the environment decides whether the recipe produces Kaito or Azure OpenAI.
