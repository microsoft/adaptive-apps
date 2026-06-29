# Challenge 04 - Build the Platform Abstractions with Recipes - Coach's Guide

[< Previous Solution](./Solution-03.md) - **[Home](./README.md)** - [Next Solution >](./Solution-05.md)

## Notes & Guidance

- This challenge is the natural continuation of Challenge 3. Teams have defined the resource type *contracts*; now they write the *implementations*. The mental shift to coach hard: **a recipe is just a Bicep file that Radius calls when a resource of a given type is deployed.**
- The two steps mirror Challenge 3: first build one recipe manually (Azure SQL) so the concept is fully understood, then register the full set of pre-built recipes in one command so teams have a working environment.
- Expected time: **60–90 minutes**. The manual SQL recipe takes 30–40 minutes. Registering and validating the pre-built recipes takes 15–20 minutes.
- Biggest coaching risk: teams start writing recipe logic before they understand the `context` object. Spend time on the `context` parameter — it is the bridge between Radius and the Bicep file.
- Do not let teams skip Azure Verified Modules (AVM). AVM is how the platform team avoids reinventing security defaults, naming, diagnostics, and tagging. If a team writes raw `Microsoft.Sql/servers` instead of reaching for AVM, redirect them.

## Key Concepts — Recipes

### What is a recipe?

A Radius **recipe** is a Bicep file (or Terraform module) that Radius calls automatically when a developer deploys a resource of a matching type. It is the *implementation* behind the type contract defined in Challenge 3.

The relationship between the three layers:

```
Resource Type (Challenge 3)          Recipe (this challenge)
──────────────────────────────        ──────────────────────────────────────────
Radius.Resources/sqlDatabases         radius/recipes/sql-server/sql-server.bicep
  input:  size, environment           → reads context.resource.properties.size
  output: host, port, database        → outputs result.values.host / port / database
  secret: password                    → outputs result.secrets.password
```

The developer never sees the recipe. They write:

```bicep
resource db 'Radius.Resources/sqlDatabases@2025-08-01-preview' = {
  properties: { environment: environment, application: app.id, size: 'S' }
}
```

Radius looks up the recipe registered for `Radius.Resources/sqlDatabases` in the current environment, calls the Bicep file, and projects the recipe's `result` output back onto `db.properties`.

### The `context` object

Every recipe receives a single `context` parameter injected by Radius. It contains everything the recipe needs without requiring the developer to pass extra parameters:

| `context` field | Contents |
|---|---|
| `context.resource.id` | Full Radius resource ID of the resource being provisioned |
| `context.resource.name` | Name given by the developer (`db`, `mqtt`, etc.) |
| `context.resource.properties` | The developer's input properties (e.g. `size`) |
| `context.environment.id` | Radius environment ID |
| `context.environment.providers.azure.scope` | Azure subscription/RG scope (when Azure provider is registered) |
| `context.runtime.kubernetes.namespace` | Kubernetes namespace for this environment |
| `context.application.name` | Application name (used for tagging/labelling) |

### Recipes and Azure Verified Modules (AVM)

A recipe calling raw ARM resource types works, but it misses security defaults, naming conventions, diagnostics, RBAC, and tagging that every Azure resource should have. **Azure Verified Modules (AVM)** are Microsoft-curated Bicep modules that encode these defaults and are tested against the Well-Architected Framework.

The pattern for every Azure recipe in this hack:

```
Recipe Bicep file  →  calls AVM module  →  provisions Azure resource
     (thin)                (thick)               (actual infra)
```

The recipe owns: parameter shaping, `context` unpacking, output mapping to Radius `result`.
AVM owns: security defaults, naming, RBAC, diagnostics, compliance tags.

Browse available AVM modules at **https://aka.ms/avm**.

### The `result` output

Every recipe must emit a `result` object. Radius reads this and:
1. Projects `result.values.*` back as read-only properties on the resource.
2. Projects `result.secrets.*` into a Kubernetes `Secret` and makes them available via the connection env-var injection mechanism.
3. Records `result.resources` as the list of Azure/Kubernetes resources the recipe created, for lifecycle tracking and deletion.

```bicep
output result object = {
  resources: [ sqlServer.outputs.resourceId, sqlDb.outputs.resourceId ]
  values: {
    host:     sqlServer.outputs.fullyQualifiedDomainName
    port:     1433
    database: databaseName
  }
  secrets: {
    password: adminPassword
  }
}
```

---

## Solution Guide

### Stage 1 — Create an Azure SQL Server recipe via the Radius dashboard

This step teaches teams to write a recipe by hand, so they understand the `context` object, the AVM pattern, and the `result` output before working with pre-built recipes.

#### Open the dashboard and navigate to Recipes

**Bash:**

```bash
rad workspace switch ws-azure-prod
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

**PowerShell:**

```powershell
rad workspace switch ws-azure-prod
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open `http://localhost:7007` in a browser.

> If your team used the sample AKS preparation values from [`prepare-aks.md`](../../common/prepare-aks.md), use that workspace and environment instead (for example `aks-trading` and `trading`).

Navigate to **Environments** → `env-azure-prod` → **Recipes**. The list is empty — no recipes have been registered yet.

#### Write the recipe Bicep

Have teams create `radius/recipes/sql-server/sql-server.bicep`. Walk through each section as they write it:

```bicep
// radius/recipes/sql-server/sql-server.bicep
// Radius Recipe for Radius.Resources/sqlDatabases (Azure SQL Server)
//
// Provisions: Azure SQL Server + single database via AVM.
// Reads:      context.resource.properties.size  → maps to SKU tier
// Outputs:    host, port (1433), database name, admin password

@description('Injected by Radius. Contains resource identity, environment scope, and developer input properties.')
param context object

@description('Database name. Defaults to the Radius resource name.')
param databaseName string = context.resource.name

// ── Size → SKU mapping ──────────────────────────────────────────────────────
var skuMap = {
  S: { name: 'GP_Gen5', tier: 'GeneralPurpose', capacity: 2 }
  M: { name: 'GP_Gen5', tier: 'GeneralPurpose', capacity: 4 }
  L: { name: 'GP_Gen5', tier: 'GeneralPurpose', capacity: 8 }
}
var sizeKey   = context.resource.properties.?size ?? 'S'
var sku       = skuMap[sizeKey]

// ── Derive names and location from context ──────────────────────────────────
var seed       = uniqueString(context.resource.id)
var serverName = 'sql-${take(seed, 10)}'

// The Azure provider scope on the environment gives us subscription + RG.
// Format: /subscriptions/<sub>/resourceGroups/<rg>
var location   = resourceGroup().location

// ── Admin password — generated deterministically from resource ID ────────────
var adminPassword = '${uniqueString(context.resource.id)}Aa1!'

// ── AVM: SQL Server ─────────────────────────────────────────────────────────
// https://aka.ms/avm — search "avm/res/sql/server"
module sqlServer 'br/public:avm/res/sql/server:0.12.0' = {
  name: 'sql-server-${seed}'
  params: {
    name:                    serverName
    location:                location
    administratorLogin:      'sqladmin'
    administratorLoginPassword: adminPassword
    databases: [
      {
        name: databaseName
        sku: {
          name: '${sku.name}_${sku.capacity}'
        }
      }
    ]
    // Security defaults provided by AVM:
    //   - TLS 1.2 minimum
    //   - Public network access can be toggled; leave default for hack
    //   - Auditing, threat detection enabled by default in AVM
    tags: {
      'radapp.io/environment': context.environment.id
      'radapp.io/resource':    context.resource.id
      'radapp.io/application': context.application == null ? '' : context.application.name
    }
  }
}

// ── Result output (required by Radius) ──────────────────────────────────────
output result object = {
  resources: [
    sqlServer.outputs.resourceId
  ]
  values: {
    host:     sqlServer.outputs.fullyQualifiedDomainName
    port:     1433
    database: databaseName
    username: 'sqladmin'
  }
  secrets: {
    password: adminPassword
  }
}
```

#### Publish the recipe

Publish the recipe Bicep to the team's ACR (OCI registry).

**For Azure Local teams:** The ACR was created during the [prepareRadius-azure-local.md](../../common/prepareRadius-azure-local.md) preparation phase. Retrieve its name from your environment and proceed to the publish command below.

**For AKS teams:** An ACR may not have been created during preparation. If needed, create one first (ACR names must be globally unique and contain only lowercase letters and numbers):

**Bash (if creating ACR):**

```bash
export RESOURCE_GROUP="<your-resource-group>"
export ACR_NAME=<globally-unique-acr-name>

az acr create \
    --name "$ACR_NAME" \
    --resource-group "$RESOURCE_GROUP" \
    --sku Basic
```

**PowerShell (if creating ACR):**

```powershell
$RESOURCE_GROUP = "<your-resource-group>"
$ACR_NAME = "<globally-unique-acr-name>"

az acr create `
    --name "$ACR_NAME" `
    --resource-group "$RESOURCE_GROUP" `
    --sku Basic
```

**Then authenticate to ACR (choose based on Docker availability):**

**Option A: If Docker is installed (Bash & PowerShell):**

```bash
az acr login -n "$ACR_NAME"
```

```powershell
az acr login -n "$ACR_NAME"
```

**Option B: If Docker is NOT installed (Docker-free authentication):**

The `--expose-token` flag returns an ACR refresh token. Use it as a password with the fixed username `00000000-0000-0000-0000-000000000000`:

**Bash:**

```bash
TOKEN=$(az acr login -n "$ACR_NAME" --expose-token -o tsv --query accessToken)
mkdir -p ~/.docker
AUTH=$(printf '00000000-0000-0000-0000-000000000000:%s' "$TOKEN" | base64 | tr -d '\n')
cat > ~/.docker/config.json <<EOF
{
  "auths": {
    "${ACR_NAME}.azurecr.io": {
      "auth": "${AUTH}"
    }
  }
}
EOF
```

**PowerShell:**

```powershell
$TOKEN = az acr login -n "$ACR_NAME" --expose-token -o tsv --query accessToken
$null = New-Item -ItemType Directory -Path "$env:USERPROFILE\.docker" -Force
$Credentials = "00000000-0000-0000-0000-000000000000:$TOKEN"
$AUTH = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($Credentials))
$ConfigPath = "$env:USERPROFILE\.docker\config.json"
@{
    auths = @{
        "$($ACR_NAME).azurecr.io" = @{
            auth = $AUTH
        }
    }
} | ConvertTo-Json | Set-Content $ConfigPath
```

**Then publish the recipe (same for both Bash and PowerShell):**

**Bash:**

```bash
rad bicep publish \
  --file radius/recipes/sql-server/sql-server.bicep \
  --target "br:${ACR_NAME}.azurecr.io/recipes/sql-server:1.0.0"
```

**PowerShell:**

```powershell
rad bicep publish `
  --file radius/recipes/sql-server/sql-server.bicep `
  --target "br:$($ACR_NAME).azurecr.io/recipes/sql-server:1.0.0"
```

#### Troubleshooting publish command formatting

If teams see:

```text
Error: required flag(s) "file", "target" not set
bash: --file: command not found
bash: --target: command not found
```

the shell parsed each line as a separate command. Use one line, or use shell-appropriate line continuation.

**Bash (one line):**

```bash
rad bicep publish --file radius/recipes/sql-server/sql-server.bicep --target "br:${ACR_NAME}.azurecr.io/recipes/sql-server:1.0.0"
```

**Bash (multiline):**

```bash
rad bicep publish \
  --file radius/recipes/sql-server/sql-server.bicep \
  --target "br:${ACR_NAME}.azurecr.io/recipes/sql-server:1.0.0"
```

**PowerShell (multiline):**

```powershell
$env:ACR_NAME="<acr-name>"
rad bicep publish `
  --file radius/recipes/sql-server/sql-server.bicep `
  --target "br:$($env:ACR_NAME).azurecr.io/recipes/sql-server:1.0.0"
```

Also ensure `ACR_NAME` is set in the same shell session before running the command.

#### Troubleshooting ACR 401 when Docker is not installed

If teams see:

```text
Unauthorized: Please login to "<acr-name>.azurecr.io"
GET "https://<acr-name>.azurecr.io/oauth2/token...": 401 unauthorized
```

the registry auth was not available to the OCI client used by `rad bicep publish`.

`az acr login -n "$ACR_NAME" --expose-token` returns a token, but does not always persist login in a way that `rad` can reuse.

Use one of these approaches:

**Option A (Docker installed):**

```bash
az acr login -n "$ACR_NAME"
```

**Option B (Docker-free): write OCI auth config from an exposed token**

The `--expose-token` flag returns an ACR refresh token. Use it as a password with the fixed username `00000000-0000-0000-0000-000000000000`:

```bash
TOKEN=$(az acr login -n "$ACR_NAME" --expose-token -o tsv --query accessToken)
mkdir -p ~/.docker
AUTH=$(printf '00000000-0000-0000-0000-000000000000:%s' "$TOKEN" | base64 | tr -d '\n')
cat > ~/.docker/config.json <<EOF
{
  "auths": {
    "${ACR_NAME}.azurecr.io": {
      "auth": "${AUTH}"
    }
  }
}
EOF
```

Then rerun publish and register commands.

Also verify RBAC: the signed-in identity needs at least `AcrPush` on the target registry.

#### Register the recipe

Register the recipe for the `sqlDatabases` type in your target environment.

If you are not sure which environment to target, list available workspaces and environments first:

```bash
rad workspace list
rad env list
```

Then register the recipe:

**Bash:**

```bash
export ENVIRONMENT_NAME="env-local-prod"  # Use your actual environment name
rad recipe register default \
    --environment "$ENVIRONMENT_NAME" \
    --resource-type Radius.Resources/sqlDatabases \
    --template-kind bicep \
    --template-path ${ACR_NAME}.azurecr.io/recipes/sql-server:1.0.0
```

**PowerShell:**

```powershell
$ENVIRONMENT_NAME = "env-local-prod"  # Use your actual environment name
rad recipe register default `
    --environment "$ENVIRONMENT_NAME" `
    --resource-type Radius.Resources/sqlDatabases `
    --template-kind bicep `
    --template-path "$($ACR_NAME).azurecr.io/recipes/sql-server:1.0.0"
```

Verify in the dashboard: navigate to **Environments** → select your environment → **Recipes** — the `Radius.Resources/sqlDatabases` entry should now appear with the template path.

Verify via CLI:

```bash
rad recipe list --environment "$ENVIRONMENT_NAME"
```

#### What to discuss

- *"Where does the recipe get the developer's `size` property?"* (`context.resource.properties.size` — the developer never passes it to the recipe directly.)
- *"Why use AVM instead of writing the raw `Microsoft.Sql/servers` resource?"* (AVM encodes TLS 1.2, auditing, tagging, naming, and other WAF defaults. The recipe stays thin and correct.)
- *"What happens to the Azure SQL Server when the developer deletes the Radius resource?"* (Radius tracks `result.resources` and deletes them in reverse order on teardown.)
- *"Could you register a second recipe for `Radius.Resources/sqlDatabases` that provisions Azure SQL Managed Instance instead?"* (Yes — register it under a different name, e.g. `managed-instance`. The developer chooses which recipe runs by naming it in their Bicep.)

---

### Stage 2 — Register the pre-built recipes for the portable app

The repository ships two environment Bicep files that define environments *and* register all recipes in a single deployment. This is the recommended pattern for a platform team: environment config and recipe registration are infrastructure-as-code, not manual CLI steps. To import the recipies the commands differ per environment type.

> **Naming note:** These commands use the Challenge 2 teaching names — group `rg-trading` with environments `env-local-prod` and `env-azure-prod`. The shipped `aks-env.bicep` defaults its `environmentName` (and Kubernetes `namespace`) parameter to `trading`, so the AKS command passes `--parameters environmentName=env-azure-prod` to align the Radius environment with the teaching names. If your team instead followed the sample values in [`prepare-aks.md`](../../common/prepare-aks.md) (`RADIUS_GROUP=trading`, `RADIUS_WORKSPACE=aks-trading`, environment `trading`), use those names consistently in every command below instead.

#### Azure Local environment (apply for local for env-local-Prod)

**Bash:**

```bash
rad deploy radius/local-env.bicep --group rg-trading --environment {env-local/azure-prod}
```

**PowerShell:**

```powershell
rad deploy radius/local-env.bicep --group rg-trading --environment {env-local/azure-prod}
```

This command deploys `local-env.bicep`, which creates the `env-local-prod` environment and registers the following recipes against `Radius.Resources/*`:

| Resource type | Recipe | Backend |
|---|---|---|
| `Radius.Resources/postgreSqlDatabases` | `postgres:latest` | PostgreSQL 16 container (Kubernetes) |
| `Radius.Resources/mqttBrokers` | `mqtt:latest` | Eclipse Mosquitto container (Kubernetes) |
| `Radius.Resources/idProviders` | `idp-keycloak:latest` | Keycloak OIDC container (Kubernetes) |
| `Radius.Resources/workloadIdentities` | `workload-identity-local:latest` | Kubernetes service account (no-op) |
| `Radius.Resources/aiModels` | `ai-agent-kaito:latest` | Kaito in-cluster LLM (Kubernetes GPU) |
| `Radius.Resources/governance` | `governance-opa:latest` | Open Policy Agent (Kubernetes) |
| `Radius.Resources/agentGuardrails` | `agent-guardrails-agt:latest` | Agent Governance Toolkit sidecar support |

#### AKS / Azure environment (apply for Azure env-azure-prod)

**Bash:**

```bash
rad deploy radius/aks-env.bicep \
    --group rg-trading \
    --environment env-azure-prod \
    --parameters environmentName=env-azure-prod \
    --parameters azureSubscriptionId=$AZURE_SUBSCRIPTION \
    --parameters azureResourceGroup=$RESOURCE_GROUP
```

**PowerShell:**

```powershell
rad deploy radius/aks-env.bicep `
    --group rg-trading `
    --environment env-azure-prod `
    --parameters environmentName=env-azure-prod `
    --parameters azureSubscriptionId=$AZURE_SUBSCRIPTION `
    --parameters azureResourceGroup=$RESOURCE_GROUP
```

This registers a parallel set of Azure-backed recipes against the same resource types:

| Resource type | Recipe | Backend |
|---|---|---|
| `Radius.Resources/postgreSqlDatabases` | `postgres:latest` | Azure Database for PostgreSQL Flexible Server (AVM) |
| `Radius.Resources/mqttBrokers` | `mqtt-azure-event-grid:latest` | Azure Event Grid MQTT namespace (AVM) |
| `Radius.Resources/workloadIdentities` | `workload-identity-azure:latest` | AKS workload identity + federated credential |
| `Radius.Resources/aiModels` | `ai-agent-azure-openai:latest` | Azure OpenAI account + deployment (AVM) |
| `Radius.Resources/governance` | `governance-opa:latest` | Open Policy Agent (Kubernetes) |
| `Radius.Resources/agentGuardrails` | `agent-guardrails-agt:latest` | Agent Governance Toolkit sidecar support |

Note that `Radius.Resources/idProviders` has no Azure recipe — Keycloak runs in-cluster in both environments. This is intentional: the portable-app pattern delegates IdP selection to the environment, and teams will replace the Keycloak recipe with a Microsoft Entra ID recipe in Challenge 6.

#### Verify in the dashboard

**Bash:**

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

**PowerShell:**

```powershell
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open `http://localhost:7007` in a browser.

Navigate to **Environments** → select the environment → **Recipes**. All registered recipe entries should appear. Click into any recipe to see the template path and kind.

Verify via CLI (same for both bash and PowerShell):

```bash
rad recipe list --environment env-azure-prod
```

#### Coaching questions

- *"Both environments register a recipe for `Radius.Resources/postgreSqlDatabases`. What is different between them?"* (The template path points to a different Bicep file. The local recipe deploys a container; the AKS recipe calls AVM to provision a managed Azure service. The resource type schema — and therefore the application Bicep — is identical.)
- *"Why is `rad deploy` used to register recipes instead of `rad recipe register`?"* (Using Bicep for environment + recipe registration is infrastructure-as-code. It is repeatable, reviewable, and version-controlled. `rad recipe register` is a CLI shortcut suitable for one-off experiments, not production.)
- *"The `governance-opa:latest` recipe is the same in both environments. When would you want different governance recipes per environment?"* (When prod uses a stricter OPA policy bundle than non-prod, or when prod routes policy decisions to an external PDP rather than running OPA in-cluster.)
- *"What is the `--group rg-trading` flag?"* (It scopes the deployment to the `rg-trading` Radius resource group — equivalent to `rad group switch rg-trading` before running `rad deploy`.)
