# Challenge 03 - Build the Platform Abstractions - Coach's Guide

[< Previous Solution](./Solution-02.md) - **[Home](./README.md)** - [Next Solution >](./Solution-04.md)

## Notes & Guidance

- This challenge is where the *platform engineering* story of Radius becomes concrete. Teams move from a running control plane to defining the **vocabulary** that application developers will use in every later challenge.
- The two steps are deliberately sequenced: first build one resource type from scratch (so the concept is understood), then import the full set of pre-built types (so teams have a realistic foundation to build on).
- Expected time: **45–75 minutes**. The manual creation step (SQL Server) takes 15–20 minutes. The import and exploration takes 10–15 minutes. The remaining time is concept discussion and validation.
- Do not let teams skip the manual creation step and go straight to the import — the learning is in understanding the schema, not just running a command.
- Typical blocker: teams confuse a **resource type** (the schema / contract) with a **recipe** (the implementation). Make sure the distinction is clear before they start. Recipes are Challenge 3+; this challenge is purely about defining the type contract.

## Key Concepts — Resource Types

Before teams open a terminal, spend 5–10 minutes landing the concept.

A Radius **resource type** is a named, versioned schema that defines:

- What **input properties** an application developer must provide when declaring a resource (e.g. `size`, `model`, `clientId`).
- What **output properties** the platform returns after a recipe runs (e.g. `host`, `port`, `endpoint`).
- What **secrets** the platform returns that should never appear in plain text (e.g. `password`, `apiKey`).

The resource type is the **contract** between the application team and the platform team. It answers the question: *"As a developer, what do I need to tell the platform, and what will the platform give me back?"*

```
Application developer writes:                 Platform returns:
──────────────────────────────────            ──────────────────────────────────
resource db 'Radius.Resources/               db.properties.host   → "sql.prod.svc"
  sqlDatabases@2025-08-01-preview' = {       db.properties.port   → 1433
  properties: {                              db.properties.secrets.password → "..."
    environment: environment
    application: app.id
    size: 'S'
  }
}
```

Key coaching points:

- Resource types live in a **namespace** (`Radius.Resources` in this repository). The namespace is purely organisational — it has no runtime effect.
- A resource type has no implementation of its own. The **recipe** (Challenge 4+) is what actually provisions infrastructure. The type just defines the shape.
- Output properties marked `readOnly: true` are written by the recipe, not the developer.
- **Connections** between resources work because resource types define a standard set of outputs. When a container declares `connections: { db: { source: db.id } }`, Radius reads the `host`, `port`, and `secrets` outputs of the target resource and injects them as `CONNECTION_DB_HOST`, `CONNECTION_DB_PORT`, etc. environment variables. The developer never hard-codes a connection string.
- The same resource type can have multiple recipes registered against it in different environments. `env-azure-prod` might register an Azure SQL recipe; `env-local-prod` might register a containerised SQL Server recipe. The application Bicep is identical in both cases.

## Solution Guide

### Stage 1 — Create a Microsoft SQL Server resource type via the Radius dashboard

This step teaches teams to define a resource type schema by hand, so they understand every field before working with pre-built types.

#### Open the dashboard and navigate to Resource Types

```bash
rad workspace switch ws-azure-prod
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open `http://localhost:7007` in a browser.

> If your team used the sample AKS preparation values from [`prepare-aks.md`](../../common/prepare-aks.md), switch to the workspace created there instead (for example `rad workspace switch aks-trading`) before opening the dashboard.

In the dashboard, navigate to **Resource Types** (left nav). The list will be empty — no custom types have been registered yet.

#### Define the schema

A resource type is defined in a YAML manifest. Have teams create `sqlDatabases.yaml` with the following content. Walk through each section as they write it:

```yaml
# sqlDatabases.yaml — Microsoft SQL Server portable resource type
namespace: Radius.Resources
types:
  sqlDatabases:
    description: A portable Microsoft SQL Server database resource.
    apiVersions:
      '2025-08-01-preview':
        schema:
          type: object
          properties:
            environment:
              type: string
              description: (Required) The Radius Environment ID.
            application:
              type: string
              description: (Required) The Radius Application ID.
            size:
              type: string
              enum: [S, M, L]
              description: (Optional) Size tier. S=2 vCores, M=4 vCores, L=8 vCores. Defaults to S.
            # Read-only outputs written by the recipe:
            host:
              type: string
              readOnly: true
              description: Hostname or IP of the SQL Server instance.
            port:
              type: integer
              readOnly: true
              description: TCP port (default 1433).
            database:
              type: string
              readOnly: true
              description: Database name.
            username:
              type: string
              readOnly: true
              description: Login username.
            secrets:
              type: object
              readOnly: true
              properties:
                password:
                  type: string
                  readOnly: true
                  description: Login password.
              required: [password]
          required: [environment, application]
```

#### Register the type via the CLI

```bash
rad resource-type create -f sqlDatabases.yaml
```

Verify it appears in the dashboard under **Resource Types** and that clicking it shows the correct schema properties.

#### What to discuss

- *"Which properties does the developer write, and which does the recipe write back?"* (input vs `readOnly`)
- *"Why is `password` nested under `secrets` rather than listed as a top-level property?"* (Radius treats `secrets` specially — it stores them in a Kubernetes `Secret`, not in the resource status.)
- *"What would change in this YAML if you wanted to support Azure SQL Managed Instance instead of SQL Server?"* (Possibly nothing in the schema — that is a recipe concern, not a type concern.)

---

### Stage 2 — Import the pre-built resource types

The repository ships a `types.yaml` file that defines all the portable resource types used by the `adaptive-apps` reference application. Teams import them with a single command and then explore what was registered.

#### Import

```bash
rad resource-type create -f radius/resource-types/types.yaml
```

> **Federated / optional two-cluster note:** Resource types are stored in the active Radius control plane. If the team has separate workspaces for `ws-local-prod` and `ws-azure-prod` (including the optional two-AKS workshop fallback), repeat this import in each workspace before moving on to recipes.

This registers the following types under the `Radius.Resources` namespace:

| Resource type | Purpose |
|---|---|
| `Radius.Resources/postgreSqlDatabases` | Portable PostgreSQL database. Recipes: containerised Postgres (local), Azure Database for PostgreSQL Flexible Server (Azure). |
| `Radius.Resources/mqttBrokers` | Portable MQTT message broker. Recipes: Eclipse Mosquitto (local), Azure Event Grid MQTT (Azure). |
| `Radius.Resources/workloadIdentities` | Workload identity configuration. Recipes: Kubernetes service account + federated credential (AKS), passthrough (local). |
| `Radius.Resources/idProviders` | OpenID Connect identity provider. Recipes: Keycloak (local/on-prem), Microsoft Entra ID (Azure). |
| `Radius.Resources/aiModels` | OpenAI-compatible AI inference endpoint. Recipes: Kaito in-cluster LLM (local), Azure OpenAI (Azure). |
| `Radius.Resources/governance` | Policy decision point / governance integration. Recipes: Open Policy Agent (local and Azure). |
| `Radius.Resources/agentGuardrails` | Agent governance sidecar prerequisites. Recipes: Agent Governance Toolkit sidecar support (local and Azure). |

#### Explore in the dashboard

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open `http://localhost:7007` in a browser.

In **Resource Types**, verify all seven types appear alongside `Radius.Resources/sqlDatabases` from Stage 1.

Click into each type and walk teams through:

- The **input properties** a developer provides (e.g. `size` on `postgreSqlDatabases`, `model` on `aiModels`, `clientId` on `workloadIdentities`).
- The **read-only outputs** a recipe writes back (e.g. `host`/`port` on `postgreSqlDatabases`, `endpoint`/`provider` on `aiModels`, `issuer`/`authEndpoint` on `idProviders`).
- The **secrets** object where sensitive values live.

#### Explore via the CLI

```bash
# List all registered resource types
rad resource-type list

# Show the schema for a specific type
rad resource-type show Radius.Resources/postgreSqlDatabases
rad resource-type show Radius.Resources/aiModels
```

#### Coaching questions

- *"What do all five types have in common in their input properties?"* (`environment` and `application` are required on every type — these are the Radius anchors that tie a resource to a deployment context.)
- *"The `aiModels` type has a `provider` output. Why would an application need to know the provider if the endpoint is already abstracted?"* (Some SDK clients — e.g. the Azure OpenAI SDK vs. the standard OpenAI SDK — behave differently. The `provider` field lets the app switch SDK configuration without changing the endpoint logic.)
- *"Could you register a recipe for `Radius.Resources/sqlDatabases` (the type you created in Stage 1) that provisions Azure SQL instead of SQL Server? What would need to change?"* (Only the recipe Bicep — the type schema stays the same.)
- *"What is the difference between `Radius.Resources/postgreSqlDatabases` and `Radius.Resources/sqlDatabases`?"* (Different engines — PostgreSQL vs. SQL Server — modelled as separate types because their output schemas differ. A recipe for one cannot fulfil the other.)

#### Validation

At the end of this challenge, `rad resource-type list` should return at least eight types:

```
Radius.Resources/agentGuardrails
Radius.Resources/aiModels
Radius.Resources/governance
Radius.Resources/idProviders
Radius.Resources/mqttBrokers
Radius.Resources/postgreSqlDatabases
Radius.Resources/sqlDatabases
Radius.Resources/workloadIdentities
```

No recipes are registered yet — that is Challenge 4. The dashboard should show each type with its schema but with an empty recipe list.
