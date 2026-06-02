# Challenge 03 - Deploy a Portable .NET 10 Web App on Azure and Azure Local with Radius - Coach's Guide

[< Previous Solution](./Solution-02.md) - **[Home](./README.md)** - [Next Solution >](./Solution-04.md)

## Notes & Guidance

In Challenge 02 the team built the *data tier* of the platform: one resource type, two recipes, identical schema, two completely different runtimes. This challenge is the **application tier counterpart**. Teams now define a `Radius.Resources/webApp` resource type for a .NET 10 web application, author one recipe that lands the app on **Azure App Service** (Linux, container-based) using an Azure Verified Module, and a second recipe that lands the same container on AKS as a `Deployment` + `Service` + `Ingress` for the **Azure Local** environment. They then wire the web app to the `Radius.Resources/postgreSQL` resource from Challenge 02 using **Radius `connections`** — without ever having to drop into VNet integration, private endpoints, or hand-rolled firewall rules.

- The single most important concept this challenge teaches: **Radius `connections` are how application authors declare data flow between resources, and the platform (recipes + Radius runtime) is responsible for actually making the network path work.** The developer writes `connections: { db: { source: pg.id } }` once. Whether that turns into an App Service connection string, a Kubernetes `EnvFrom`, a firewall rule, or a private endpoint is a *platform* decision, not an *application* decision. This is the payoff for everything they did in Challenge 02.
- Second most important concept: **the .NET 10 app does not change between Azure and Azure Local.** Same container image, same `appsettings.json`, same env var names. If a team finds themselves writing `#if AZURE` or branching on environment in `Program.cs`, stop them — that is the anti-pattern this whole hack is fighting.
- Expected time to complete for a team: **3–4.5 hours**. Rough split:
  - `webApp` resource type manifest: 20–30 minutes (cheaper than the postgreSQL one because they have done it once).
  - Azure recipe (App Service via AVM, container-based): 60–90 minutes — most of it is waiting for App Service to pull and start the image. Tell them to start the AKS recipe in parallel.
  - Azure Local recipe (`Deployment` + `Service` + `Ingress`): 45–60 minutes.
  - Networking stage (wiring the web app to the database via `connections`, plus the Azure-side firewall rule and the AKS-side `NetworkPolicy`): 45–60 minutes. **This is the stage where teams learn the most — do not let them skip it or merge it into the recipes.**
  - End-to-end smoke test (browse to the app, see records from Postgres): 20–30 minutes.
- Suggested wait before stepping in: **20–25 minutes of no forward progress on the Azure recipe**, **15 minutes on the AKS recipe**, **10 minutes on the networking step** (the networking step has a much smaller surface area; if they are stuck for 10 minutes they are usually stuck on a single concept and a Socratic question unblocks them faster than reading).
- Do NOT let teams cheat by exposing the database publicly to "make the app work." The whole point of the networking stage is that they should reach the database **without** opening it to the internet. If you see `0.0.0.0/0` in a firewall rule outside the Azure recipe's `AllowAzureServices` rule from Challenge 02, push back.

### Key concepts the coach must land before the challenge starts

- **Radius `connection`** — a *declarative dependency edge* from one Radius resource (the consumer, the web app) to another (the provider, the database). When Radius processes a connection, it does three things:
  1. Reads the `values` and `secrets` outputs of the target resource (from Challenge 02: `host`, `port`, `database`, `username`, `password`, `connectionString`).
  2. Projects them into the consumer recipe as the `context.connections.<name>` object, and (by convention) into the running container as **environment variables** with a deterministic naming convention (`CONNECTION_<NAME>_<KEY>`, e.g. `CONNECTION_DB_CONNECTIONSTRING`). Recipes are free to rename / remap these — see the sample web app recipe.
  3. Triggers any **runtime networking** the recipe registers — for the Azure recipe that means a per-IP firewall rule on the Postgres flexible server allowing the App Service's outbound IPs; for the AKS recipe it means a Kubernetes `NetworkPolicy` that allows pod-to-pod traffic on port 5432 only from the consumer's pods.
  - Connections are the seam where "application code" ends and "platform plumbing" begins. Coach this hard.
- **Azure App Service for Containers** — managed PaaS that runs a container image you point at. For this hack we use the **Linux** plan with a **container deployment**. Two reasons: (1) the same image runs unchanged on AKS, and (2) App Service for Linux supports `WEBSITES_PORT`, simple environment variables, managed identity, and outbound IP egress without VNet integration — which is exactly what we need to avoid wiring up a VNet just to talk to Postgres.
- **AVM for App Service** — `br/public:avm/res/web/site:<version>` plus `br/public:avm/res/web/serverfarm:<version>`. The `serverfarm` AVM provisions the App Service Plan; the `site` AVM provisions the Web App on top. As with Challenge 02: have teams check `https://aka.ms/avm` for the latest version on the day of the event rather than hard-coding from this guide.
- **Bicep `kubernetes` provider** — same provider as Challenge 02. The Azure Local recipe declares a `Deployment`, a `ClusterIP` `Service`, and an `Ingress` (NGINX or whatever ingress controller is installed on the team's AKS in Challenge 01). No Helm chart, no `kubectl apply`.
- **Networking without a VNet (the heart of this challenge)**:
  - On **Azure**, the App Service has a stable set of **outbound public IPs** (`possibleOutboundIpAddresses` on the `Microsoft.Web/sites` resource). The Postgres flexible server already allows "AllowAzureServices" from Challenge 02, which would technically be enough — but the platform team should treat that rule as a workshop crutch and **layer per-app firewall rules** keyed off the App Service outbound IPs *from the recipe*, not by clicking in the portal. No VNet, no private endpoint, no DNS zone work.
  - On **Azure Local / AKS**, traffic is in-cluster: the web app pod resolves `<dbresource>.<namespace>.svc.cluster.local` and connects on port 5432. A `NetworkPolicy` restricts who can talk to the DB pod — the policy allows traffic only from pods labeled with the consumer's `radapp.io/resource` value. No VNet exists, and no VNet is needed.
- **Why this matters as a teaching moment**: most teams' default mental model for "web app talks to managed database in Azure" is "create a VNet, integrate App Service into it, create a private endpoint on the database, manage a private DNS zone." That stack is correct for production but it is **massive overkill for a portable hack workload** and it actively *prevents* portability — a private endpoint architecture is meaningless in Azure Local. The lesson: lean on the platform's built-in egress + identity primitives until you have a reason not to.

### Who does what: app code vs. resource type vs. recipe vs. Radius runtime

| Concern | Owned by | Why |
|---|---|---|
| Reading `CONNECTION_DB_CONNECTIONSTRING` and connecting to Postgres | **The .NET 10 app** | The app only sees env vars; it does not know if it is on App Service, AKS, or anything else. |
| Declaring that the app *needs* a `postgreSQL` named `db` | **`Radius.Resources/webApp` resource (in the app Bicep) via `connections`** | This is the developer's contract. |
| Declaring the *schema* of a portable web app (image, port, env, connections, scaling) | **`Radius.Resources/webApp` resource type manifest** | Same role as the postgres RT in Challenge 02. |
| Provisioning the Azure App Service Plan + Web App + container settings | **Azure recipe (Bicep + AVM)** | Hides Azure specifics from the developer. |
| Provisioning the Kubernetes `Deployment` / `Service` / `Ingress` | **Azure Local recipe (Bicep `kubernetes` provider)** | Same role, different runtime. |
| Generating the firewall rule that lets App Service reach Postgres | **Azure recipe** (per-connection) | The recipe knows the App Service's outbound IPs; the developer should not. |
| Generating the `NetworkPolicy` that lets the web app pod reach the DB pod | **Azure Local recipe** (per-connection) | The recipe knows the namespace and the workload labels. |
| Resolving `connections.db.source` to the actual `host` / `port` / `connectionString` of the Postgres resource | **Radius runtime** | Radius reads the target resource's outputs and injects them into the consumer recipe. |
| Naming, identity, diagnostics, TLS, https-only, FTPS-disable, minimum TLS version | **AVM** | Microsoft's defaults are good; do not re-implement. |

If a team asks "why is the firewall rule in the consumer's recipe and not in Challenge 02's recipe?" the answer is: in Challenge 02 the database does not yet know which web apps will consume it. The firewall rule is a **per-connection** concern, not a per-database concern. Authoring it in the consumer's recipe (or in a dedicated networking sub-module) keeps lifecycle correct: when the web app is deleted, its firewall rule is deleted too. Putting it on the database would either be too permissive ("AllowAzureServices") or too coupled ("the DB knows about every app").

### Typical blockers to watch for

- **App boots but immediately 502s on App Service.** Almost always one of:
  - `WEBSITES_PORT` not set, or set to something other than the port the .NET 10 app listens on (default `8080` for `mcr.microsoft.com/dotnet/aspnet:10.0` minimal images, but `5000` for some scaffold templates).
  - Image pulls fail because the App Service plan's outbound is being blocked, or because the team forgot to enable managed-identity-based ACR pulls on the Web App. The AVM enables managed identity by default — if they leave it off, App Service falls back to admin user / password and most teams have admin disabled on ACR.
  - The container exits because `ASPNETCORE_URLS` is set to `https://+:443` (some templates do this) but App Service terminates TLS in front — the container should listen on **HTTP** on a single port.
- **Connection from web app to Postgres times out on Azure.** Outbound IP set has changed (App Service can rotate it on plan scale events) or the team is on a plan tier that does not publish stable outbound IPs (Free/Shared). Push them to Basic or Standard.
- **Connection from web app to Postgres fails on Azure Local with `getaddrinfo ENOTFOUND`.** They are using `localhost` or hard-coded the DB hostname in `appsettings.json`. They should be reading `CONNECTION_DB_HOST` (or `CONNECTION_DB_CONNECTIONSTRING`) — that is the *whole point* of the connection abstraction.
- **`NetworkPolicy` makes everything fail on AKS.** The team's AKS may not have a CNI that enforces `NetworkPolicy` (kubenet ignores it; Azure CNI Overlay with Cilium / Calico enforces it). Either fix the cluster (Challenge 01 should have already used a CNI that supports policy) or make the policy advisory and call out the gap — but do not skip it silently.
- **Two recipes diverge on env var naming.** `CONNECTION_DB_CONNECTIONSTRING` on Azure, `connection_db_connectionstring` on AKS. Bicep is case-sensitive in env var keys and so is .NET configuration on Linux. Stick to upper-snake-case in both recipes.
- **Web app recipe creates an App Service Plan per app.** Fine for a hack, but call out in coaching that production patterns share a plan across many apps. The recipe should accept an optional `appServicePlanId` parameter so the platform team can pre-provision shared plans — make sure they design for that even if they do not implement it.
- **Ingress on AKS does not route.** Either no ingress controller is installed (Challenge 01 should have installed one) or the `Ingress` resource references the wrong `ingressClassName`. Have the team `kubectl get ingressclass` first.
- **Treating networking as part of "deploy"** — i.e. doing the firewall rule by hand after `rad deploy`. The whole challenge is broken if the networking is not declarative. If the team is `az` / `kubectl`-ing their way to a working app, gently steer them back to the recipe.

## Solution Guide

Walk teams through the four stages below in order. Stage 4 (networking) is intentionally a *separate step* even though it logically lives inside the recipes — pulling it out makes the conceptual model crisper, and lets teams see what changes between "app deploys" and "app actually reaches the DB." For a real platform team you would fold the networking back into the per-runtime recipes after the workshop; the sample manifests at the end of this guide already show that fully integrated form.

### Stage 1 — Author the `Radius.Resources/webApp` resource type

Same shape as Challenge 02, but for an HTTP application instead of a database. The manifest defines:

- The **type name** `Radius.Resources/webApp` and the API version (use the same convention the team picked in Challenge 02 — typically `2025-01-01-preview`).
- The **input properties**:
  - `image` (string, required) — fully-qualified container image reference, e.g. `myacr.azurecr.io/contoso/web:1.0.0`. The image must contain the .NET 10 app; how it got built is out of scope for this challenge (point teams at the `Student/Resources/src/` folder for a sample app and a Dockerfile based on `mcr.microsoft.com/dotnet/aspnet:10.0`).
  - `port` (int, optional, default `8080`) — the HTTP port the container listens on.
  - `env` (object, optional) — additional environment variables the app needs.
  - `connections` (object, optional) — map of named dependencies onto other Radius resources. For this challenge the only connection is `db` → `Radius.Resources/postgreSQL`. Each connection has a `source` (the target resource's id) and an optional `disableDefaultEnvVars` flag.
  - `replicas` (int, optional, default `1`) — desired replica / instance count. Recipes map this to App Service plan worker count or Kubernetes Deployment replicas respectively.
- The **outputs**:
  - `url` (string) — the public URL of the app. On Azure it is `https://<app-name>.azurewebsites.net`. On Azure Local it is `https://<app-name>.<ingress-domain>` from the cluster's ingress controller.
  - `internalHost` (string) — useful for debugging and for downstream services that want to reach this app inside the cluster / inside the platform.
- **Capabilities**: `SupportsRecipes: true`, plus — and this is new compared to Challenge 02 — `SupportsConnections: true`. This is what tells Radius that resources of this type can declare `connections` and expect Radius to inject env vars / secrets. Without it, the connection block is silently ignored.

Publish it with the same `rad resource-type create` flow they used in Challenge 02. They should now have *two* user-defined resource types in the environment: `Radius.Resources/postgreSQL` from Challenge 02 and `Radius.Resources/webApp` from this challenge.

A reference manifest is included at the end of this guide ([see sample manifests](#sample-manifests)).

### Stage 2 — Author the Azure recipe (App Service for Containers via AVM)

The Azure recipe is a Bicep file that:

1. Declares parameters that mirror the resource type's input properties (`image`, `port`, `env`, `replicas`), and accepts the standard `context` object from Radius.
2. Calls the **AVM for App Service Plan** (`br/public:avm/res/web/serverfarm:<version>`) to provision a **Linux** plan, SKU `B1` for hack budgets (point out that Free/Shared do not publish stable outbound IPs and break the networking stage). The plan name is derived from `context.environment.id` so multiple apps in the same environment can share it — but for hack simplicity it is fine to provision one plan per app, *as long as the recipe accepts an optional `appServicePlanId` parameter that, when provided, skips the plan creation and reuses an existing one.* Coach this design discussion explicitly.
3. Calls the **AVM for App Service site** (`br/public:avm/res/web/site:<version>`) with:
   - `kind: 'app,linux,container'`
   - `serverFarmResourceId`: the plan id from step 2 (or the `appServicePlanId` parameter).
   - `siteConfig.linuxFxVersion`: `'DOCKER|${image}'`.
   - `siteConfig.appSettings`: the union of:
     - `WEBSITES_PORT` set to `port`.
     - `ASPNETCORE_URLS` set to `http://+:${port}` (override whatever the image baked in).
     - The caller-supplied `env` map.
     - **Connection-derived env vars** — see Stage 4. For now, leave a placeholder.
   - `siteConfig.acrUseManagedIdentityCreds: true` and `managedIdentities: { systemAssigned: true }` so the Web App pulls from ACR using its managed identity. The recipe must also create a **role assignment** granting the Web App's principalId `AcrPull` on the ACR. AVM's `site` module supports a `roleAssignments` parameter on companion AVM modules — alternatively, drop down to a raw `Microsoft.Authorization/roleAssignments` resource.
   - `httpsOnly: true`, `minimumTlsVersion: '1.2'`, FTPS disabled. The AVM defaults to these, so as long as teams do not override them they get them for free.
4. Emits Radius outputs:

    ```bicep
    output result object = {
      resources: [
        plan.outputs.resourceId
        site.outputs.resourceId
      ]
      values: {
        url:          'https://${site.outputs.defaultHostName}'
        internalHost: site.outputs.defaultHostName
      }
      // App Service has no per-app connection-secret to project; secrets are
      // injected as App Service application settings inline (see Stage 4).
    }
    ```

5. Publish + register exactly like Challenge 02, but for the new resource type and the `azure` environment:

    ```bash
    rad bicep publish \
        --file ./recipes/azure/webapp.bicep \
        --target br:${acr_name}.azurecr.io/recipes/webapp-azure:1.0.0
    rad recipe register default \
        --environment azure \
        --resource-type Radius.Resources/webApp \
        --template-kind bicep \
        --template-path ${acr_name}.azurecr.io/recipes/webapp-azure:1.0.0
    ```

**Division of responsibilities for this recipe:**

- The **recipe file** (≈ 60 lines): typed parameters; AVM calls; managed-identity + ACR role assignment; output shaping. *No* networking yet — that is Stage 4.
- The **AVM**: HTTPS-only, TLS 1.2 floor, FTPS off, app insights wiring (if turned on), naming compliance, default tags, optional staging slot, optional autoscale.
- **Radius**: looking up the recipe, passing `context` and the resource properties in, projecting `result.values` back onto the resource so the next pipeline stage / CI can read `app.url`.

### Stage 3 — Author the Azure Local recipe (container on AKS with Ingress)

"Azure Local" continues to mean *just Kubernetes* — the same AKS cluster Challenge 01 set up. The recipe is a Bicep file using the `kubernetes` provider that:

1. Imports the `kubernetes` provider scoped to `context.runtime.kubernetes.namespace` (do not hard-code the namespace).
2. Declares a `Deployment` of `replicas` pods running the supplied `image`, with `containerPort` = `port`, `envFrom` for the connection-derived secret (Stage 4), inline `env` for non-secret config, and a **readiness probe** on `GET /healthz` (or whatever the team's app exposes — the sample app exposes `/healthz`). Resource requests/limits should be modest (`100m` CPU / `128Mi` memory request, `500m` / `512Mi` limit).
3. Declares a `ClusterIP` `Service` exposing `port: 80` → `targetPort: <port>`. ClusterIP because the Ingress will terminate the public traffic.
4. Declares an `Ingress` — `apiVersion: networking.k8s.io/v1` — with:
   - `ingressClassName` derived from the team's installed ingress controller (`nginx` is the most common choice; `webapprouting.kubernetes.azure.com` if they used the AKS App Routing add-on).
   - A host derived from `context.resource.name` plus the cluster's wildcard ingress domain (which Challenge 01 should have stashed somewhere — typically as an environment-level config value the recipe receives as a parameter).
   - TLS via the cluster's default cert / cert-manager.
5. Emits the same `result` schema as the Azure recipe:
   - `url` = `https://<host>` from the ingress.
   - `internalHost` = `<service-name>.<namespace>.svc.cluster.local`.
   - Same `resources` list (Deployment, Service, Ingress).

Publish + register against the `azure-local` environment, same flow as Challenge 02's local recipe.

**Division of responsibilities for this recipe:**

- No AVM — Kubernetes resources are declared directly in Bicep.
- Bicep `kubernetes` provider lays down the `Deployment`, `Service`, and `Ingress` via the cluster's API server.
- Radius handles invocation, output projection, and lifecycle.
- The `Ingress` is **not** authoritative for TLS — that is delegated to the cluster's ingress controller and cert-manager. Calling this out keeps the recipe portable across clusters that use different cert plumbing.

### Stage 4 — Networking: connect the web app to the database (no VNet required)

This is the stage the rest of the challenge exists to set up. Teams handle networking in **three layers**, all declarative, all per-environment, **none requiring a VNet in Azure**.

**Layer A — Application code: read connection env vars only.**

The .NET 10 app reads its database connection from `CONNECTION_DB_CONNECTIONSTRING` (or, equivalently, the discrete `CONNECTION_DB_HOST`, `CONNECTION_DB_PORT`, etc., env vars Radius injects). The app does not care which environment it is in. Show teams:

```csharp
var pgConnString = builder.Configuration["CONNECTION_DB_CONNECTIONSTRING"]
    ?? throw new InvalidOperationException("Missing CONNECTION_DB_CONNECTIONSTRING");
builder.Services.AddDbContext<AppDb>(o => o.UseNpgsql(pgConnString));
```

If a team has hard-coded `Server=localhost;Database=hackdb;...` in `appsettings.json`, **stop them now**. The connection string is platform-supplied at runtime.

**Layer B — The application Bicep declares the connection.**

In the application Bicep (which the developer writes — the same file they will extend in Challenge 04, but a stripped-down version is fine here), the web app references the database via a Radius `connection`:

```bicep
extension radius

param environment string
param image       string
param dbId        string  // resource id of the Radius.Resources/postgreSQL from Challenge 02

resource web 'Radius.Resources/webApp@v1alpha1' = {
  name: 'contoso-web'
  properties: {
    environment: environment
    image:       image
    port:        8080
    connections: {
      db: {
        source: dbId
      }
    }
  }
}
```

When Radius processes this resource, it looks up the `db` connection, reads the postgres resource's `values` and `secrets`, and injects them into the recipe as `context.connections.db`. The recipe then shapes those into env vars (App Service appSettings, or a Kubernetes `Secret` + `envFrom`) using the `CONNECTION_<NAME>_<KEY>` naming convention. Both recipes must do this — it is the contract that makes the app portable.

**Layer C — Each recipe registers the runtime networking primitive that allows the path.**

This is the *new* code that did not exist in Challenge 02:

- **Azure recipe** — *no VNet, no private endpoint*. Instead:
  1. After the AVM for `site` returns, read `site.outputs.outboundIpAddresses` (a comma-separated list of stable outbound IPs for the App Service Plan).
  2. For each IP, declare a `Microsoft.DBforPostgreSQL/flexibleServers/firewallRules` resource that allows `<ip>/32`. The flexible-server resource id was passed in via the connection — it is on `context.connections.db.target`. The recipe attaches the firewall rule **scoped to the Postgres resource group**, not the web app's resource group, so cleanup follows the right resource.
  3. Add the firewall rules to the recipe's `result.resources` so they are deleted when the web app is deleted. **This is the lifecycle property that makes per-connection firewall rules safe — they go away when the consumer goes away.**
  4. *Why this works without a VNet*: App Service for Linux on Basic/Standard or higher has a stable, published outbound IP set. A managed Postgres flexible server with public access enabled accepts connections from those IPs once the firewall rules exist. TLS (`sslmode=require`) protects the traffic. Coach: this is intentionally a *minimum-viable* networking story for a hack; production uses VNet integration + private endpoint, but the *Radius abstraction is the same* — you would just swap the firewall rule resource for a private endpoint resource inside the recipe.
- **Azure Local recipe** — runtime networking is in-cluster:
  1. The web app pod is labeled `radapp.io/resource: ${context.resource.id}` (already happens in the Deployment).
  2. The recipe declares a `NetworkPolicy` in the application namespace that:
     - `podSelector`: matches the database pod's labels (the Postgres recipe from Challenge 02 already labels its Deployment with `radapp.io/resource=<postgres-id>`).
     - `ingress.from.podSelector`: matches the web app pod's `radapp.io/resource` label.
     - `ingress.ports`: TCP `5432`.
  3. Add the `NetworkPolicy` to `result.resources` so it is cleaned up with the web app.
  4. *Why this works without a VNet*: there is no VNet on Azure Local — the cluster network is the network. CNI-enforced `NetworkPolicy` is the right primitive for "only this app can talk to this database." If the team's AKS does not enforce policies, push them back to fix Challenge 01's cluster setup, do not paper over it.

After Layer C, do **not** also expose the database publicly. If a team has both the `NetworkPolicy` (allow only the web app) and a `Service` of type `LoadBalancer` on the database, they have built two contradictory networking rules. The correct shape is: DB is `ClusterIP` only; the only thing publicly reachable is the web app's `Ingress` / `Web App`.

**Coaching question to land Stage 4:** ask the team "what changed in the *application's* code or Bicep when we added networking?" The answer is **nothing** — the developer wrote the connection in Layer B and stopped there. Everything else was platform code. That is the win.

### Stage 5 — End-to-end smoke test

Success criteria:

1. `rad recipe list --environment azure` shows recipes for both `Radius.Resources/postgreSQL` (from Challenge 02) and `Radius.Resources/webApp` (from this challenge). Same for `azure-local`.
2. Deploy the application Bicep (web app + reference to the postgres resource) against `azure`:
   - `rad deploy ./app.bicep -p image=...`
   - `rad resource show Radius.Resources/webApp contoso-web` returns a public `url`.
   - Browse to the URL — the app's home page renders, and a page that lists records from Postgres returns data (the sample app should run a one-time `EnsureCreated` + seed if the DB is empty).
   - In the Azure portal: confirm there is **no VNet** in the resource group, **no private endpoint** on the Postgres server, but there *are* per-IP firewall rules on the Postgres server matching the App Service's outbound IPs.
3. Switch to `azure-local`, deploy the same Bicep, repeat the smoke test. The app's URL is now an ingress-served hostname; the database is in-cluster.
4. On `azure-local`, verify the `NetworkPolicy` is in effect: `kubectl exec` into a *different* pod in the same namespace and try to `nc -vz <pg-service> 5432` — it must time out. From the web app pod, the same command must succeed.
5. Delete the web app on each environment and confirm:
   - Azure: per-app firewall rules on the Postgres server are gone; the App Service site and plan are gone; the Postgres server (owned by Challenge 02) is **still there**.
   - AKS: the web app's `Deployment`, `Service`, `Ingress`, and the `NetworkPolicy` are gone; the Postgres `Deployment` is still there.

If all five succeed, the team has a fully portable .NET 10 web app that runs unchanged on managed Azure App Service or on AKS, talks to a portable Postgres database from Challenge 02, and routes traffic via per-environment networking primitives — **with no VNet anywhere**.

## Sample manifests

Hand these out only if a team is badly stuck. The learning is in writing them.

### `webApp.yaml` — resource type manifest

```yaml
# Radius user-defined resource type: a portable HTTP web application.
# Two recipes implement it (Azure App Service for Containers, and a
# Deployment+Service+Ingress on AKS for Azure Local).
namespace: Radius.Resources
types:
  webApp:
    description: A portable HTTP web application implemented by recipes.
    capabilities: [ "SupportsRecipes", "SupportsConnections" ]
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
            image:
              type: string
              description: Fully-qualified container image reference.
            port:
              type: integer
              default: 8080
              description: HTTP port the container listens on.
            env:
              type: object
              additionalProperties: { type: string }
              description: Additional environment variables.
            replicas:
              type: integer
              default: 1
              description: Desired replica / instance count.
            connections:
              type: object
              additionalProperties:
                type: object
                properties:
                  source: { type: string }
                  disableDefaultEnvVars: { type: boolean, default: false }
              description: Named connections to other Radius resources.
            # Read-only outputs projected back by the recipe.
            url:
              type: string
              readOnly: true
            internalHost:
              type: string
              readOnly: true
          required: [ environment, image ]
```

### `recipes/azure/webapp.bicep` — Azure recipe (App Service for Containers via AVM, with per-connection firewall rules)

```bicep
// Azure recipe for Radius.Resources/webApp.
// Provisions: an App Service Plan (Linux, B1), a Web App (container), a
// system-assigned managed identity, an AcrPull role assignment on the team's
// ACR, and per-connection PostgreSQL firewall rules for each App Service
// outbound IP.
//
// Owns:     image deployment, identity wiring, env vars, networking glue.
// Does NOT own: image build, ACR creation, the Postgres server itself.

@description('Injected by Radius. Contains resource / env / app identity AND connections.')
param context object

@description('Fully-qualified container image reference.')
param image string

@description('HTTP port the container listens on.')
param port int = 8080

@description('Additional environment variables.')
param env object = {}

@description('Desired replica count. App Service maps to plan worker count.')
param replicas int = 1

@description('Optional pre-provisioned App Service Plan id. Skip plan creation when set.')
param appServicePlanId string = ''

@description('Resource id of the team ACR; needed to grant AcrPull to the Web App identity.')
param acrResourceId string

var location = resourceGroup().location

var seed     = uniqueString(context.resource.id)
var appName  = toLower('web${take(seed, 10)}')
var planName = 'plan-${uniqueString(context.environment.id)}'

// 1. App Service Plan (Linux). Skipped if an existing plan id is supplied.
module plan 'br/public:avm/res/web/serverfarm:0.4.1' = if (empty(appServicePlanId)) {
  name: 'plan-${planName}'
  params: {
    name:        planName
    location:    location
    skuName:     'B1'
    skuCapacity: replicas
    kind:        'Linux'
    reserved:    true
  }
}

var resolvedPlanId = empty(appServicePlanId) ? plan.outputs.resourceId : appServicePlanId

// 2. Build the appSettings array, including connection-derived env vars.
//    Radius surfaces connections under context.connections.<name>.values / .secrets.
var connectionEnv = [for c in items(context.?connections ?? {}): {
  name:  toUpper('CONNECTION_${c.key}_CONNECTIONSTRING')
  value: c.value.secrets.connectionString
}]

var staticEnv = [for k in items(env): {
  name:  k.key
  value: k.value
}]

var coreEnv = [
  { name: 'WEBSITES_PORT',   value: string(port) }
  { name: 'ASPNETCORE_URLS', value: 'http://+:${port}' }
]

// 3. The Web App.
module site 'br/public:avm/res/web/site:0.12.0' = {
  name: 'site-${appName}'
  params: {
    name:                 appName
    location:             location
    kind:                 'app,linux,container'
    serverFarmResourceId: resolvedPlanId
    httpsOnly:            true
    managedIdentities:    { systemAssigned: true }
    siteConfig: {
      linuxFxVersion:             'DOCKER|${image}'
      acrUseManagedIdentityCreds: true
      minTlsVersion:              '1.2'
      ftpsState:                  'Disabled'
      appSettings:                union(coreEnv, staticEnv, connectionEnv)
    }
    tags: {
      'radapp.io/environment': context.environment.id
      'radapp.io/resource':    context.resource.id
    }
  }
}

// 4. Grant the Web App's managed identity AcrPull on the team ACR.
resource acrPull 'Microsoft.Authorization/roleAssignments@2022-04-01' = {
  name:  guid(acrResourceId, site.outputs.systemAssignedMIPrincipalId, 'AcrPull')
  scope: tenantResourceId('Microsoft.ContainerRegistry/registries', last(split(acrResourceId, '/')))
  properties: {
    principalId:      site.outputs.systemAssignedMIPrincipalId
    principalType:    'ServicePrincipal'
    roleDefinitionId: subscriptionResourceId(
      'Microsoft.Authorization/roleDefinitions',
      '7f951dda-4ed3-4680-a7ca-43fe172d538d') // AcrPull
  }
}

// 5. Networking (Layer C): per-connection firewall rules on the Postgres server.
//    Only fires when a connection named "db" is wired to a Postgres resource
//    whose target resource id points at a flexibleServers instance.
var hasDbConnection  = contains(context.?connections ?? {}, 'db')
var pgServerResource = hasDbConnection ? context.connections.db.target : ''
var outboundIps      = split(site.outputs.outboundIpAddresses, ',')

module fwRules 'modules/pg-firewall-rules.bicep' = if (hasDbConnection) {
  name:  'fw-${appName}'
  scope: resourceGroup(split(pgServerResource, '/')[4]) // pg server's RG
  params: {
    pgServerName:   last(split(pgServerResource, '/'))
    ruleNamePrefix: appName
    allowedIps:     outboundIps
  }
}

// 6. Outputs.
output result object = {
  resources: union(
    [ site.outputs.resourceId ],
    empty(appServicePlanId) ? [ plan.outputs.resourceId ] : [],
    hasDbConnection ? fwRules.outputs.firewallRuleIds : []
  )
  values: {
    url:          'https://${site.outputs.defaultHostName}'
    internalHost: site.outputs.defaultHostName
  }
}
```

The companion `modules/pg-firewall-rules.bicep` module just iterates the IP list and emits one `Microsoft.DBforPostgreSQL/flexibleServers/firewallRules` per IP, returning the resource ids in an array. Keeping it in its own module makes the lifecycle list cleaner.

### `recipes/local/webapp.bicep` — Azure Local recipe (Deployment + Service + Ingress + NetworkPolicy)

```bicep
// Azure Local / on-cluster recipe for Radius.Resources/webApp.
// Runs the container as a Deployment behind a ClusterIP Service and an Ingress.
// Adds a NetworkPolicy in the same namespace allowing only this web app's pods
// to reach the database pod on TCP/5432.

extension kubernetes with {
  namespace:  context.runtime.kubernetes.namespace
  kubeConfig: ''
} as k8s

param context object
param image    string
param port     int    = 8080
param env      object = {}
param replicas int    = 1

@description('Cluster-wide ingress wildcard domain, e.g. apps.aks-eastus.example.com')
param ingressDomain string

@description('IngressClass installed on the cluster (e.g. nginx, webapprouting.kubernetes.azure.com)')
param ingressClassName string = 'nginx'

var appName    = toLower(replace(context.resource.name, '_', '-'))
var hostName   = '${appName}.${ingressDomain}'
var labelKey   = 'radapp.io/resource'
var labelValue = context.resource.id

// 1. Connection env values: surface every connection's connectionString as
//    a Secret entry the Deployment will envFrom. Same naming convention as
//    the Azure recipe.
var connectionSecretData = toObject(
  [for c in items(context.?connections ?? {}): {
    key:   toUpper('CONNECTION_${c.key}_CONNECTIONSTRING')
    value: c.value.secrets.connectionString
  }],
  e => e.key, e => e.value)

resource connSecret 'core/Secret@v1' = if (!empty(context.?connections ?? {})) {
  metadata: { name: '${appName}-conn' }
  stringData: connectionSecretData
}

// 2. Deployment.
resource deploy 'apps/Deployment@v1' = {
  metadata: {
    name: appName
    labels: {
      app:           appName
      '${labelKey}': labelValue
    }
  }
  spec: {
    replicas: replicas
    selector: { matchLabels: { app: appName } }
    template: {
      metadata: { labels: { app: appName, '${labelKey}': labelValue } }
      spec: {
        containers: [
          {
            name:  'app'
            image: image
            ports: [ { containerPort: port } ]
            env:   [for k in items(env): { name: k.key, value: k.value }]
            envFrom: empty(context.?connections ?? {}) ? [] : [
              { secretRef: { name: '${appName}-conn' } }
            ]
            readinessProbe: {
              httpGet: { path: '/healthz', port: port }
              initialDelaySeconds: 5
              periodSeconds: 5
            }
            resources: {
              requests: { cpu: '100m', memory: '128Mi' }
              limits:   { cpu: '500m', memory: '512Mi' }
            }
          }
        ]
      }
    }
  }
}

// 3. Service.
resource svc 'core/Service@v1' = {
  metadata: { name: appName }
  spec: {
    type:     'ClusterIP'
    selector: { app: appName }
    ports: [ { port: 80, targetPort: port } ]
  }
}

// 4. Ingress.
resource ing 'networking.k8s.io/Ingress@v1' = {
  metadata: { name: appName }
  spec: {
    ingressClassName: ingressClassName
    rules: [
      {
        host: hostName
        http: {
          paths: [
            {
              path:     '/'
              pathType: 'Prefix'
              backend:  { service: { name: svc.metadata.name, port: { number: 80 } } }
            }
          ]
        }
      }
    ]
    tls: [
      { hosts: [ hostName ], secretName: '${appName}-tls' }
    ]
  }
}

// 5. Networking (Layer C): NetworkPolicy on the DB pod allowing only this app.
//    Only emitted when a "db" connection is present.
var hasDb        = contains(context.?connections ?? {}, 'db')
var dbResourceId = hasDb ? context.connections.db.target : ''

resource dbPolicy 'networking.k8s.io/NetworkPolicy@v1' = if (hasDb) {
  metadata: { name: '${appName}-to-db' }
  spec: {
    podSelector: {
      matchLabels: {
        '${labelKey}': dbResourceId
      }
    }
    policyTypes: [ 'Ingress' ]
    ingress: [
      {
        from: [
          { podSelector: { matchLabels: { '${labelKey}': labelValue } } }
        ]
        ports: [ { protocol: 'TCP', port: 5432 } ]
      }
    ]
  }
}

// 6. Outputs.
output result object = {
  resources: union(
    [
      '/planes/kubernetes/local/namespaces/${context.runtime.kubernetes.namespace}/providers/apps/Deployment/${deploy.metadata.name}'
      '/planes/kubernetes/local/namespaces/${context.runtime.kubernetes.namespace}/providers/core/Service/${svc.metadata.name}'
      '/planes/kubernetes/local/namespaces/${context.runtime.kubernetes.namespace}/providers/networking.k8s.io/Ingress/${ing.metadata.name}'
    ],
    empty(context.?connections ?? {}) ? [] : [
      '/planes/kubernetes/local/namespaces/${context.runtime.kubernetes.namespace}/providers/core/Secret/${appName}-conn'
    ],
    hasDb ? [
      '/planes/kubernetes/local/namespaces/${context.runtime.kubernetes.namespace}/providers/networking.k8s.io/NetworkPolicy/${appName}-to-db'
    ] : []
  )
  values: {
    url:          'https://${hostName}'
    internalHost: '${svc.metadata.name}.${context.runtime.kubernetes.namespace}.svc.cluster.local'
  }
}
```

### `app.bicep` — sample application Bicep the developer writes

```bicep
extension radius

@description('Radius environment id (e.g. /planes/radius/local/.../environments/azure-local).')
param environment string

@description('Container image reference for the .NET 10 web app.')
param image string

@description('Resource id of the Radius.Resources/postgreSQL from Challenge 02.')
param dbId string

resource web 'Radius.Resources/webApp@v1alpha1' = {
  name: 'contoso-web'
  properties: {
    environment: environment
    image:       image
    port:        8080
    replicas:    1
    connections: {
      db: { source: dbId }
    }
  }
}

output url string = web.properties.url
```

### End-to-end publish, register & deploy script

```bash
# Assumes Challenge 01 + 02 are complete. Reuses $acr_name and the two
# environments "azure" and "azure-local". Assumes the .NET 10 image has
# already been pushed to ACR (image build is out of scope for this challenge).

set -euo pipefail

# 1. Publish the resource type
rad resource-type create Radius.Resources/webApp \
    --from-file ./resource-types/webApp.yaml

az acr login -n "$acr_name"

# 2. Publish + register the Azure recipe
rad bicep publish \
    --file  ./recipes/azure/webapp.bicep \
    --target "br:${acr_name}.azurecr.io/recipes/webapp-azure:1.0.0"
rad recipe register default \
    --environment    azure \
    --resource-type  Radius.Resources/webApp \
    --template-kind  bicep \
    --template-path  "${acr_name}.azurecr.io/recipes/webapp-azure:1.0.0" \
    --parameters     acrResourceId="$(az acr show -n "$acr_name" --query id -o tsv)"

# 3. Publish + register the Azure Local recipe
rad bicep publish \
    --file  ./recipes/local/webapp.bicep \
    --target "br:${acr_name}.azurecr.io/recipes/webapp-local:1.0.0"
rad recipe register default \
    --environment    azure-local \
    --resource-type  Radius.Resources/webApp \
    --template-kind  bicep \
    --template-path  "${acr_name}.azurecr.io/recipes/webapp-local:1.0.0" \
    --parameters     ingressDomain="apps.${cluster_dns_zone}" ingressClassName="nginx"

# 4. Deploy and smoke test against both environments.
image="${acr_name}.azurecr.io/contoso/web:1.0.0"

for env in azure azure-local; do
  echo "=== Deploying contoso-web to $env ==="
  rad env switch "$env"

  # Resource id of the postgres deployed in Challenge 02 in this environment.
  dbId="$(rad resource show Radius.Resources/postgreSQL contoso-db --output json | jq -r .id)"

  rad deploy ./app.bicep \
    -p environment="$(rad env show -o json | jq -r .id)" \
    -p image="$image" \
    -p dbId="$dbId"

  url="$(rad resource show Radius.Resources/webApp contoso-web --output json | jq -r .properties.url)"
  echo "App is at: $url"
  curl -fsS "$url/healthz"
  curl -fsS "$url/items" | head -c 400 ; echo

  rad resource delete Radius.Resources/webApp contoso-web --yes
done
```

After the script completes successfully against both environments, the team has demonstrated the full Radius platform-engineering loop: a developer-facing resource type, two runtime recipes that are completely different under the hood, and per-environment networking that is declared *as part of the connection*, not as a separate VNet design exercise. They are now ready to add additional services and connections in the next challenge.
