# Challenge 02 - Define a PostgreSQL Resource Type and Author Recipes for Azure and Azure Local - Coach's Guide

[< Previous Solution](./Solution-01.md) - **[Home](./README.md)** - [Next Solution >](./Solution-03.md)

## Notes & Guidance

This challenge is where the *platform engineering* story of Radius really clicks for attendees: they move from consuming a pre-built environment (Challenge 01) to **defining a new abstraction** (a `postgreSQL` resource type) and then **wiring up two very different implementations of that abstraction** — one that calls Azure to provision a managed Azure Database for PostgreSQL flexible server, and one that runs PostgreSQL as a container on AKS for an "Azure Local" / on-premises-style environment. The application developer in later challenges will consume *one* resource type and never know (or care) which implementation was chosen.

- The single most important concept to get across before teams start coding: **Radius resource types are a contract; recipes are the implementation.** An application author writes `resource db 'Radius.Resources/postgreSQL@v1alpha1'` once. The platform engineer picks which recipe runs based on the *environment* the app is deployed to. The code the developer writes does not change between Azure and Azure Local.
- Second most important concept: **Bicep is the language, Azure Verified Modules (AVM) are pre-built building blocks, and a Radius recipe is the orchestrator that glues them together, passes parameters in, and returns outputs back to Radius.** Teams routinely conflate the three — clarify this early, ideally with the mini-lecture in `Lectures.pptx` before they open a terminal.
- Expected time to complete for a team: **2.5–4 hours**. Rough split:
  - Resource Type manifest and schema: 30–45 minutes.
  - Azure recipe (Bicep + AVM for Azure DB for PostgreSQL flexible server): 45–75 minutes (most of it is waiting for the Azure deployment — tell them to parallelize and start the Kubernetes recipe while Azure is provisioning).
  - Kubernetes / "Azure Local" recipe: 45–60 minutes.
  - Registration, environment wiring, and end-to-end smoke test: 30–45 minutes.
- Suggested wait before stepping in: **20 minutes of no forward progress**. The most common silent-stuck state is teams reading the Radius docs back-to-front rather than iterating on a recipe — push them to run `rad bicep publish` / `rad recipe register` early and often, even against a half-written recipe, because the feedback loop is much faster than reading.
- Do NOT let teams deploy an actual application from this challenge. The success criterion is that `rad recipe show default postgres` returns both recipes and that a throwaway test `Radius.Resources/postgreSQL` resource can be created and deleted in both environments. Application wiring is the next challenge.

### Key concepts the coach must land before the challenge starts

- **Radius Resource Type (RT)** — an environment-scoped, versioned, declarative type definition (`kind: ResourceType` in YAML) that describes *what properties a resource has* and *what outputs it returns*. It is the API surface the application developer codes against. A resource type has **no implementation of its own**; it simply says "anyone who claims to implement `Radius.Resources/postgreSQL@v1alpha1` must accept `databaseName`, `version`, `storageGB`, etc., and must return `host`, `port`, `database`, and a `connectionSecret`."
- **Recipe** — a named, environment-scoped *implementation* of a resource type. A recipe is either a **Bicep template** (for Azure or Azure-compatible providers) or a **Terraform module**. When an app is deployed and Radius sees a `Radius.Resources/postgreSQL` resource, it looks up the recipe registered on the current environment for that type, invokes it with the resource's properties as parameters, and copies the recipe's outputs back onto the resource. The recipe is invoked by the Radius **deployment engine** (`bicep-de` pod in `radius-system`) — teams do not call `az deployment` themselves.
- **Bicep** — the DSL the recipe is written in. Bicep is a declarative, compile-to-ARM language. In Radius, Bicep has two jobs: (1) describe the Azure resources to be created (via ARM / AVM), and (2) describe the **outputs** that Radius will project back onto the resource. Bicep is *also* the language recipes use to emit **Kubernetes** manifests via the `kubernetes` provider — that is how the Azure Local recipe is written without needing Helm or raw `kubectl`.
- **Azure Verified Module (AVM)** — a Microsoft-authored, community-reviewed Bicep (or Terraform) module that implements best practices for a single Azure resource family (naming, diagnostics, private endpoints, identity, tagging, etc.). Using an AVM instead of hand-rolled `Microsoft.DBforPostgreSQL/flexibleServers` resources is the difference between "quick demo" and "I could ship this." The AVM for PostgreSQL Flexible Server is published to the Bicep public registry at `br/public:avm/res/db-for-postgre-sql/flexible-server:<version>` — teams should always use the **latest** version published to the registry at the time of the hack (have them check `https://aka.ms/avm` on the day of the event rather than hard-coding a version from this guide).

### Who does what: Bicep vs. AVM vs. Radius recipe

This is the question attendees ask most. A concrete answer to give them:

| Concern | Owned by | Why |
|---|---|---|
| *Declaring* what a PostgreSQL looks like to the app developer (properties, outputs, required vs optional, defaults) | **Radius Resource Type manifest** | This is the public contract. It is independent of Azure, Kubernetes, or any cloud. |
| Running the right implementation for the current environment | **Radius (control plane)** — specifically the recipe engine | The app author does not choose; the environment's recipe binding does. |
| Executing the implementation template and returning outputs | **Radius deployment engine (`bicep-de`)** | Radius invokes Bicep with the resource's properties as parameters. |
| Describing *what Azure resources to create* and *how they are wired* | **Bicep** | The language the recipe is authored in. Bicep compiles to ARM and is submitted by `bicep-de` to Azure Resource Manager. |
| Implementing best-practice Azure resource configuration (private networking, diagnostics, identity, HA, tagging, naming) | **Azure Verified Module** | AVMs package Microsoft's opinionated best practices. The recipe simply calls `module pg 'br/public:avm/...' = { ... }` and sets a few parameters. |
| Describing Kubernetes workloads (Deployment, Service, Secret) for the Azure Local recipe | **Bicep with the `kubernetes` provider** | Same Bicep recipe engine, different target — no Helm chart or `kubectl apply` needed. |
| Creating the `connectionSecret` that the app will later bind to | **Recipe** (whether Azure or Kubernetes) | Radius recipes support a `result` output with a `secrets` object; Radius turns that into a Kubernetes `Secret` in the app's namespace automatically. |

If a team asks "why can't the application developer just write Bicep directly?" the answer is: they could, but then (a) the app is coupled to Azure and cannot run on Azure Local without a full rewrite, (b) every developer re-invents private endpoints / networking / identity, and (c) the platform team has no central place to enforce policy (e.g., "all prod Postgres has geo-redundant backup"). The resource type + recipe split puts those decisions in the platform team's hands exactly once per environment.

### Typical blockers to watch for

- **Resource type not showing up.** After `rad resource-type create`, teams forget to switch to the UCP resource group where they published it, or they try to reference the RT in a recipe before it is registered. Have them run `rad resource-type list` and confirm the type exists before authoring recipes.
- **Recipe compiles locally but fails in `bicep-de`.** Nearly always one of:
  - The Bicep file references a module from `br/public:avm/...` but the `bicep-de` pod cannot reach the public Bicep registry. This manifests as `Failed to restore external modules`. Check egress from the AKS cluster.
  - The recipe declares outputs that do not match the resource type schema. Radius is strict: every property on `status.outputResources` must be declared; every secret must be under `result.values.secrets`. Have them diff against the resource type manifest.
  - AVM parameter name drift between versions. AVM modules are versioned independently and parameters occasionally change. If they copy-paste from a blog post, they almost certainly have a stale parameter name.
- **"It worked for Alice and not Bob."** Recipes are environment-scoped, not workspace-scoped. If team members are on different `rad env` values they will see different recipes. Have everyone run `rad env show` and confirm they are on the same environment.
- **Kubernetes recipe pulls the Postgres image and gets rate-limited by Docker Hub.** Use `mcr.microsoft.com/azurelinux/base/postgres:16` or push a copy to the team's ACR from Challenge 01 and reference it there.
- **Secrets not landing in the app namespace.** The recipe must emit secrets via `output result object = { values: { secrets: { ... } } }`. Writing a Kubernetes `Secret` directly in Bicep "works" but Radius will not treat it as a resource output and later challenges that bind `db.connectionSecret` will fail.
- **Using a Radius `postgresql` built-in.** Older Radius versions shipped a `Applications.Datastores/postgresql` built-in resource type. That is **not** what this challenge is about. Teams must create their own user-defined resource type — it is the whole point of Radius today (built-ins are being deprecated in favor of user-defined types).

## Solution Guide

Walk teams through the four stages below in order. Do not let them publish a recipe until the resource type is in place — otherwise they will chase output-schema errors for an hour.

### Stage 1 — Author the `Radius.Resources/postgreSQL` resource type

The resource type is a single YAML manifest. It defines:

- The **type name** (`Radius.Resources/postgreSQL`) and the API **version** (`2025-01-01-preview` or `v1alpha1` — either naming convention is acceptable, but be consistent across the team).
- The **input properties** the application developer will set. For this hack we recommend:
  - `databaseName` (string, required) — the logical database to create inside the server.
  - `version` (string, optional, default `"16"`) — major version of Postgres.
  - `storageGB` (int, optional, default `32`) — initial storage.
  - `skuName` (string, optional, default `"Standard_B1ms"`) — ignored by the Kubernetes recipe; consumed by the Azure recipe.
  - `highAvailability` (bool, optional, default `false`) — ignored by the Kubernetes recipe.
  - `environment` and `application` — inherited from the base Radius resource contract; do not redeclare them.
- The **outputs** every recipe for this type must return:
  - `host` (string)
  - `port` (int)
  - `database` (string)
  - `username` (string)
  - A connection secret containing `password` and a full `connectionString`. Secrets go in a dedicated `secrets` block in the schema so they are never exposed in plain-text resource state.
- The **capabilities** block should include `SupportsRecipes: true` so Radius knows this type is recipe-backed (as opposed to core-implemented).

Commit this manifest into the repo under `Student/Resources/resource-types/postgreSQL.yaml` so it is shareable. Publish it with:

```bash
rad resource-type create Radius.Resources/postgreSQL \
    --from-file ./postgreSQL.yaml
rad resource-type show Radius.Resources/postgreSQL
```

A reference resource type manifest is included at the end of this guide ([see sample manifests](#sample-manifests)). If a team cannot get past this stage in 45 minutes, hand it to them and move on — this is not the hard part of the challenge.

### Stage 2 — Author the Azure recipe (managed Azure Database for PostgreSQL flexible server via AVM)

The Azure recipe is a Bicep file that:

1. Declares parameters that **match the resource type's input properties** (Radius passes them in by name). It also receives a `context` object from Radius that contains `resource.name`, `environment.id`, `application.id`, and similar metadata — this is how the recipe uniquely names Azure resources per deployment.
2. Calls the **AVM for Azure Database for PostgreSQL Flexible Server** module: `br/public:avm/res/db-for-postgre-sql/flexible-server:<version>`. Parameters to set:
   - `name`: derive from `context.resource.name` plus a short hash of `context.environment.id` to keep DNS names unique.
   - `location`: the resource group location.
   - `skuName` / `tier`: from the recipe parameter.
   - `version`: from the recipe parameter.
   - `storageSizeGB`: from the recipe parameter.
   - `highAvailability`: set `{ mode: 'ZoneRedundant' }` when the input is `true`, `{ mode: 'Disabled' }` otherwise.
   - `administratorLogin` and `administratorLoginPassword`: generate the password inside the recipe using `newGuid()` seeded with `context.resource.id` so it is stable on re-deployment (otherwise every redeploy rotates the password and breaks the app). Document this: do not let teams invent their own scheme.
   - `databases`: pass `[ { name: databaseName } ]` so the AVM provisions the database alongside the server.
   - `firewallRules`: for a hack, allow Azure services (`0.0.0.0` to `0.0.0.0`). In a real deployment this would be a private endpoint and VNet integration — call this out explicitly to attendees so they do not copy the hack config to production.
3. Emits Radius outputs. The shape Radius expects is:

    ```bicep
    output result object = {
      // Resources Radius should consider "owned" by this recipe for lifecycle management.
      resources: [
        '${pg.outputs.resourceId}'
      ]
      values: {
        host:     pg.outputs.fqdn
        port:     5432
        database: databaseName
        username: administratorLogin
      }
      secrets: {
        password:         administratorLoginPassword
        connectionString: 'postgresql://${administratorLogin}:${administratorLoginPassword}@${pg.outputs.fqdn}:5432/${databaseName}?sslmode=require'
      }
    }
    ```

    - Everything under `values` becomes readable properties on the `Radius.Resources/postgreSQL` resource.
    - Everything under `secrets` becomes a Kubernetes `Secret` in the application's namespace, named after the resource, and mounted/projected into consuming containers by Radius.
    - The `resources` list drives **lifecycle**: when the Radius resource is deleted, the deployment engine deletes the listed Azure resources too.

4. Publish and register the recipe:

    ```bash
    # Publish the compiled Bicep to an OCI registry (the team's ACR from Challenge 01).
    rad bicep publish \
        --file ./recipes/azure/postgres.bicep \
        --target br:${acr_name}.azurecr.io/recipes/postgres-azure:1.0.0

    # Bind the recipe to the Azure environment under the name "default".
    rad recipe register default \
        --environment azure \
        --resource-type Radius.Resources/postgreSQL \
        --template-kind bicep \
        --template-path ${acr_name}.azurecr.io/recipes/postgres-azure:1.0.0
    ```

    Names matter: registering a recipe called `default` makes it the recipe used when the application resource does not specify a recipe by name. That is what we want — the app stays portable.

**Division of responsibilities in this recipe, spelled out explicitly for the coach:**

- The **recipe file itself** (the Bicep you author) is about **40 lines**. It:
  - Accepts typed parameters.
  - Calls the AVM.
  - Shapes the outputs into the Radius-expected `result` object.
- The **AVM** is several hundred lines of Bicep that you do not write and do not maintain. It:
  - Names the flexible server in compliance with Azure naming rules.
  - Provisions the server, the database, the firewall rule, optional private endpoint, optional diagnostic settings, optional managed identity for Entra authentication.
  - Applies Microsoft's default tags and supports tag override.
- **Radius** is responsible for:
  - Looking up which recipe to run (`default` on environment `azure` for type `Radius.Resources/postgreSQL`).
  - Passing properties and `context` into the recipe as Bicep parameters.
  - Submitting the Bicep to Azure Resource Manager via `bicep-de`.
  - Taking the recipe's `result` outputs and projecting them onto the Radius resource's `status` and into the Kubernetes `Secret` the application will mount.
  - Tracking the resources the recipe declared it owns and deleting them when the Radius resource is deleted.

### Stage 3 — Author the Azure Local recipe (PostgreSQL container on AKS)

"Azure Local" in this hack means: **no Azure data plane dependency** — the database is a container that runs on the same AKS cluster hosting the Radius control plane (or on an Azure Local / Arc-connected Kubernetes cluster in a real deployment — the recipe is identical because both are "just Kubernetes"). This is also the configuration teams should use for local dev and for test environments where they do not want to wait 5 minutes per recipe run for Azure to provision.

The recipe is a Bicep file that targets Bicep's `kubernetes` provider. It:

1. Imports the `kubernetes` provider, scoped to the namespace Radius passes in via `context.runtime.kubernetes.namespace`. This is crucial: the recipe *must not* hard-code a namespace, because Radius environments are namespace-scoped and recipes are reused across namespaces.
2. Generates a deterministic admin password the same way the Azure recipe does (seeded from `context.resource.id`).
3. Declares three Kubernetes resources:
   - A `Secret` holding `POSTGRES_USER`, `POSTGRES_PASSWORD`, and `POSTGRES_DB`.
   - A `Deployment` that runs `mcr.microsoft.com/azurelinux/base/postgres:16` (or the ACR mirror if the cluster cannot reach MCR), mounts an `emptyDir` at `/var/lib/postgresql/data`, references the `Secret` as `envFrom`, requests 500m CPU / 512Mi memory, exposes port 5432, and includes a readiness probe that runs `pg_isready`. For non-dev environments a `PersistentVolumeClaim` would replace the `emptyDir` — call this out as the first thing to change before using it for anything real.
   - A `Service` of type `ClusterIP` exposing port 5432, so the app inside the cluster can reach it by `<resource-name>.<namespace>.svc.cluster.local`.
4. Emits the same `result` object as the Azure recipe, but with `host` set to the in-cluster DNS name, `port: 5432`, `username: postgres`, and the same `password` / `connectionString` secret shape. Because both recipes return the **same schema**, the application is unchanged between environments.

Publish and register identically to Stage 2, but against the **Azure Local** environment (`azure-local` or whatever the team named it in Challenge 01):

```bash
rad bicep publish \
    --file ./recipes/local/postgres.bicep \
    --target br:${acr_name}.azurecr.io/recipes/postgres-local:1.0.0

rad recipe register default \
    --environment azure-local \
    --resource-type Radius.Resources/postgreSQL \
    --template-kind bicep \
    --template-path ${acr_name}.azurecr.io/recipes/postgres-local:1.0.0
```

**Division of responsibilities in this recipe:**

- There is **no AVM** involved in the Kubernetes case — AVMs are Azure Resource Manager modules. Instead, the recipe directly declares the three Kubernetes resources in Bicep.
- **Bicep** is still the language: the Bicep `kubernetes` provider turns the `resource foo 'apps/Deployment@v1' = {...}` syntax into Kubernetes API calls, which `bicep-de` makes against the cluster's API server.
- **Radius** is responsible for the same things it is in the Azure case, minus the Azure data plane: it invokes the recipe, projects outputs, and tracks lifecycle. Specifically, when the Radius resource is deleted, Radius deletes the `Deployment`, `Service`, and `Secret` the recipe declared in its `result.resources` list.
- If you want an apples-to-apples "module" equivalent, point teams at the Radius community recipes repository (`radius-project/recipes` on GitHub) — the Postgres-on-Kubernetes recipe there is the same pattern, just more polished.

### Stage 4 — Register the recipes on both environments and smoke test

By this point the team has:

- One resource type: `Radius.Resources/postgreSQL`.
- Two recipes registered as `default` on two environments: `azure` and `azure-local`.

Smoke test (the success criteria for the challenge):

1. `rad recipe list --environment azure` shows the Azure recipe.
2. `rad recipe list --environment azure-local` shows the Kubernetes recipe.
3. `rad recipe show --environment azure default --resource-type Radius.Resources/postgreSQL` returns the recipe's parameters pulled from the compiled Bicep — this is a quick way to confirm the Bicep is well-formed and uploaded correctly.
4. Deploy a **throwaway test resource** (no application wiring):

    ```bash
    cat > /tmp/pg-test.bicep <<'EOF'
    extension radius
    resource pg 'Radius.Resources/postgreSQL@v1alpha1' = {
      name: 'smoketest'
      properties: {
        environment: environment().id
        databaseName: 'hackdb'
      }
    }
    EOF

    # Against the Azure environment
    rad env switch azure
    rad deploy /tmp/pg-test.bicep
    rad resource show Radius.Resources/postgreSQL smoketest
    rad resource delete Radius.Resources/postgreSQL smoketest --yes

    # Against the Azure Local environment
    rad env switch azure-local
    rad deploy /tmp/pg-test.bicep
    kubectl get deploy,svc,secret -n <app-namespace> -l radapp.io/resource=smoketest
    rad resource delete Radius.Resources/postgreSQL smoketest --yes
    ```

5. On both environments, confirm:
   - `rad resource show` exposes `host`, `port`, `database`, `username`.
   - A Kubernetes `Secret` named after the resource exists in the app namespace and contains `password` and `connectionString`.
   - After `rad resource delete`, the Azure flexible server (or the K8s Deployment/Service/Secret) is actually gone. Lifecycle is as important as creation for a good recipe.

If all five succeed against both environments, the challenge is complete. The team now has a platform abstraction that an application can consume unchanged against either Azure or Azure Local.

## Sample manifests

Hand these out only if a team is badly stuck — the learning is in writing them, not in pasting them.

### `postgreSQL.yaml` — resource type manifest

```yaml
# Radius user-defined resource type: a portable PostgreSQL database.
# Application authors only ever interact with this schema. The two recipes
# (Azure managed flexible server, and containerized on AKS) implement it.
namespace: Radius.Resources
types:
  postgreSQL:
    description: A portable PostgreSQL database implemented by recipes.
    capabilities: [ "SupportsRecipes" ]
    apiVersions:
      "2025-01-01-preview":
        schema:
          type: object
          properties:
            environment:
              type: string
              description: The Radius environment ID. Required.
            application:
              type: string
              description: The Radius application ID. Optional.
            databaseName:
              type: string
              description: Logical database to create inside the server.
            version:
              type: string
              default: "16"
              description: PostgreSQL major version.
            storageGB:
              type: integer
              default: 32
              description: Initial storage in GB.
            skuName:
              type: string
              default: "Standard_B1ms"
              description: Azure SKU. Ignored by the Kubernetes recipe.
            highAvailability:
              type: boolean
              default: false
              description: Enable zone-redundant HA. Ignored by the Kubernetes recipe.
            # Read-only outputs projected back by the recipe.
            host:
              type: string
              readOnly: true
            port:
              type: integer
              readOnly: true
            database:
              type: string
              readOnly: true
            username:
              type: string
              readOnly: true
          required: [ environment, databaseName ]
```

### `recipes/azure/postgres.bicep` — Azure recipe using the AVM

```bicep
// Azure recipe for Radius.Resources/postgreSQL.
// Owns:     calling the AVM, stable-password generation, output shaping.
// Does NOT own: naming, diagnostics, HA topology, firewall policy — the AVM does.

@description('Injected by Radius. Contains resource / env / app identity.')
param context object

@description('Logical database name to create on the server.')
param databaseName string

@description('PostgreSQL major version.')
param version string = '16'

@description('Initial storage in GB.')
param storageGB int = 32

@description('Azure SKU name.')
param skuName string = 'Standard_B1ms'

@description('Enable zone-redundant HA.')
param highAvailability bool = false

var location = resourceGroup().location

// Deterministic names & password derived from the Radius resource id so that
// re-running the recipe for the same resource does not rotate the password
// or rename the server.
var seed      = uniqueString(context.resource.id)
var serverName = toLower('pg${take(seed, 10)}')
var adminLogin = 'radadmin'
var adminPassword = 'P${seed}!${uniqueString(context.environment.id)}'

module pg 'br/public:avm/res/db-for-postgre-sql/flexible-server:0.11.2' = {
  name: 'pg-${serverName}'
  params: {
    name:                        serverName
    location:                    location
    skuName:                     skuName
    tier:                        startsWith(skuName, 'Standard_B') ? 'Burstable' : 'GeneralPurpose'
    version:                     version
    storageSizeGB:               storageGB
    administratorLogin:          adminLogin
    administratorLoginPassword:  adminPassword
    highAvailability:            highAvailability ? 'ZoneRedundant' : 'Disabled'
    databases: [
      { name: databaseName }
    ]
    firewallRules: [
      {
        name:           'AllowAzureServices'
        startIpAddress: '0.0.0.0'
        endIpAddress:   '0.0.0.0'
      }
    ]
    tags: {
      'radapp.io/environment': context.environment.id
      'radapp.io/resource':    context.resource.id
    }
  }
}

// Shape of the outputs is fixed by the Radius recipe contract.
output result object = {
  resources: [
    pg.outputs.resourceId
  ]
  values: {
    host:     '${serverName}.postgres.database.azure.com'
    port:     5432
    database: databaseName
    username: adminLogin
  }
  secrets: {
    password:         adminPassword
    connectionString: 'postgresql://${adminLogin}:${adminPassword}@${serverName}.postgres.database.azure.com:5432/${databaseName}?sslmode=require'
  }
}
```

### `recipes/local/postgres.bicep` — Azure Local recipe using the Bicep `kubernetes` provider

```bicep
// Azure Local / on-cluster recipe for Radius.Resources/postgreSQL.
// Runs PostgreSQL as a container on AKS. Intended for dev, test,
// and air-gapped / Azure Local scenarios where no Azure data plane is available.
//
// NOTE: uses emptyDir for storage — swap in a PersistentVolumeClaim for anything
// beyond a hack.

extension kubernetes with {
  namespace:  context.runtime.kubernetes.namespace
  kubeConfig: ''
} as k8s

param context object
param databaseName string
param version string = '16'

// storageGB / skuName / highAvailability are accepted to keep the schema
// identical to the Azure recipe, but are intentionally ignored here.
param storageGB int = 32
param skuName string = ''
param highAvailability bool = false

var appName      = toLower(replace(context.resource.name, '_', '-'))
var seed         = uniqueString(context.resource.id)
var adminPassword = 'P${seed}!'

resource pgSecret 'core/Secret@v1' = {
  metadata: {
    name: '${appName}-pg'
  }
  stringData: {
    POSTGRES_USER:     'postgres'
    POSTGRES_PASSWORD: adminPassword
    POSTGRES_DB:       databaseName
  }
}

resource pgDeployment 'apps/Deployment@v1' = {
  metadata: {
    name: '${appName}-pg'
    labels: {
      app: '${appName}-pg'
      'radapp.io/resource': context.resource.id
    }
  }
  spec: {
    replicas: 1
    selector: { matchLabels: { app: '${appName}-pg' } }
    template: {
      metadata: { labels: { app: '${appName}-pg' } }
      spec: {
        containers: [
          {
            name:  'postgres'
            image: 'mcr.microsoft.com/azurelinux/base/postgres:${version}'
            ports: [ { containerPort: 5432 } ]
            envFrom: [ { secretRef: { name: pgSecret.metadata.name } } ]
            readinessProbe: {
              exec: { command: [ 'pg_isready', '-U', 'postgres' ] }
              initialDelaySeconds: 5
              periodSeconds: 5
            }
            resources: {
              requests: { cpu: '250m', memory: '256Mi' }
              limits:   { cpu: '500m', memory: '512Mi' }
            }
            volumeMounts: [
              { name: 'data', mountPath: '/var/lib/postgresql/data' }
            ]
          }
        ]
        volumes: [
          { name: 'data', emptyDir: {} }
        ]
      }
    }
  }
}

resource pgService 'core/Service@v1' = {
  metadata: {
    name: '${appName}-pg'
  }
  spec: {
    type: 'ClusterIP'
    selector: { app: '${appName}-pg' }
    ports: [ { port: 5432, targetPort: 5432 } ]
  }
}

var host = '${pgService.metadata.name}.${context.runtime.kubernetes.namespace}.svc.cluster.local'

output result object = {
  resources: [
    '/planes/kubernetes/local/namespaces/${context.runtime.kubernetes.namespace}/providers/apps/Deployment/${pgDeployment.metadata.name}'
    '/planes/kubernetes/local/namespaces/${context.runtime.kubernetes.namespace}/providers/core/Service/${pgService.metadata.name}'
    '/planes/kubernetes/local/namespaces/${context.runtime.kubernetes.namespace}/providers/core/Secret/${pgSecret.metadata.name}'
  ]
  values: {
    host:     host
    port:     5432
    database: databaseName
    username: 'postgres'
  }
  secrets: {
    password:         adminPassword
    connectionString: 'postgresql://postgres:${adminPassword}@${host}:5432/${databaseName}?sslmode=disable'
  }
}
```

### End-to-end publish & register script

```bash
# Assumes Challenge 01 is complete: $acr_name, two environments "azure" and
# "azure-local" exist, and the current workspace points at the shared AKS cluster.

set -euo pipefail

# 1. Publish the resource type
rad resource-type create Radius.Resources/postgreSQL \
    --from-file ./resource-types/postgreSQL.yaml

# Authenticate Bicep to the ACR used as the recipe registry.
az acr login -n "$acr_name"

# 2. Publish + register the Azure recipe
rad bicep publish \
    --file  ./recipes/azure/postgres.bicep \
    --target "br:${acr_name}.azurecr.io/recipes/postgres-azure:1.0.0"
rad recipe register default \
    --environment    azure \
    --resource-type  Radius.Resources/postgreSQL \
    --template-kind  bicep \
    --template-path  "${acr_name}.azurecr.io/recipes/postgres-azure:1.0.0"

# 3. Publish + register the Azure Local / Kubernetes recipe
rad bicep publish \
    --file  ./recipes/local/postgres.bicep \
    --target "br:${acr_name}.azurecr.io/recipes/postgres-local:1.0.0"
rad recipe register default \
    --environment    azure-local \
    --resource-type  Radius.Resources/postgreSQL \
    --template-kind  bicep \
    --template-path  "${acr_name}.azurecr.io/recipes/postgres-local:1.0.0"

# 4. Smoke test against both environments
for env in azure azure-local; do
  echo "=== Smoke test against $env ==="
  rad env switch "$env"
  rad deploy ./tests/pg-smoketest.bicep
  rad resource show Radius.Resources/postgreSQL smoketest
  rad resource delete Radius.Resources/postgreSQL smoketest --yes
done
```

After the script completes successfully against both environments, the coach can mark the challenge as complete and the team is ready to consume `Radius.Resources/postgreSQL` from an actual application in the next challenge — without ever having to know whether they are running on managed Azure or on a containerized database on AKS.
