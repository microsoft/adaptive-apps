# Challenge 05 - Port the App Across Environments - Coach's Guide

[< Previous Solution](./Solution-04.md) - **[Home](./README.md)** - [Next Solution >](./Solution-06.md)

> This guide has two parts. **Sections 1-9 are a facilitation guide** - use them to help
> teams reason their way to a working second deployment rather than handing them answers.
> The **Detailed Solution Walkthrough** at the end is validated ground truth (exact commands,
> expected outcomes, troubleshooting) for confirming a team's work, unblocking a stuck team,
> or running the closing demo. Lead with facilitation; reach for the walkthrough only after
> the hint ladder is exhausted.

## 1. Challenge Overview for Coaches

In Challenges 3 and 4, teams built a stable application-facing contract (`Radius.Resources/*`) and registered different recipes behind that contract. Challenge 5 is where the portability promise has to become visible: teams deploy the same `radius/app.bicep` application model to a second environment and let the environment select the backing services.

The scenario frames this as a business request. Adaptive Apps already runs the trading app in one environment and now needs a second one (typically an Azure/AKS production environment alongside the existing local or Azure Local environment). Leadership's claim is that "it is the same app, so a second environment should not be a rebuild." The team's job is to prove or qualify that claim.

**Intended learning outcomes.** By the end, a team should be able to:

- Move one application model into two environments without maintaining two copies of it.
- Point precisely at what changed between the environments and explain why.
- Separate application-team concerns from platform-team concerns when describing the port.

**Key technologies and concepts.** Radius application model, environments, recipes, workspaces, and resource groups; the resource-type contract as a portability seam; Kubernetes across two targets (local/Azure Local and AKS); Azure-backed capabilities such as Event Grid MQTT, workload identity, and Azure OpenAI.

**Time.** Expect 45-75 minutes if both environments were prepared in earlier challenges. Add time if a second cluster, Azure credentials, or an Azure resource group still need to be provisioned.

## 2. Learning Objectives Breakdown

The student guide asks teams to work through a small number of open-ended tasks. Map each one to what it is actually developing.

| Student task | Skills developed | Concepts reinforced | What "good" looks like |
|---|---|---|---|
| Decide on two environments and keep context aligned | Operational discipline with control planes | Workspace vs cluster context; the deployment target is two settings, not one | The team can state where a deployment will land before running it, and never has to guess afterward |
| Confirm each environment is ready before deploying | Reading platform state | Environments map capabilities to recipes; readiness is a precondition | The team inspects registered recipes first and treats a gap as an environment problem |
| Move the same app model to both environments | Parameterization over duplication | Application model is invariant; difference lives in target plus parameters | `radius/app.bicep` is byte-identical across both deployments |
| Investigate which capabilities are backed differently | Tracing a contract to its implementation | Recipes are the seam; the app asks for capabilities, not products | The team can name which capabilities changed and where that change is expressed |
| Explain what changed and who owned it | Communicating an architecture boundary | App-team vs platform-team ownership | The team can give a leadership-ready answer separating app params from platform changes |

## 3. Expected Solution Approaches (High-Level)

There is no single correct path. Expect and accept variation. Keep this section conceptual when coaching; do not turn it into instructions.

- **Two clusters, two workspaces (strongest demo).** Local/Azure Local first, then AKS/Azure second. This gives the cleanest environment split and the most honest production/non-production story. Most teams with the prerequisites in place will land here.
- **One cluster, two Radius environments/groups (fallback).** Teams with only one cluster can still practice the pattern by creating a second Radius environment and deploying to a different Radius group. This demonstrates the control-plane model but is weaker as a production split; coach them to name that limitation rather than hide it.
- **Difference carried by parameters vs. carried by the environment file.** Some teams will reach for deployment parameters, others will point at the environment Bicep and its recipe selections. Both are legitimate; the teaching point is that neither requires touching the application model.
- **Postgres stays containerized in both environments.** A team may expect a managed Azure database to appear in Azure. In the current repository it does not, because both environment files keep the Kubernetes PostgreSQL recipe. That is still portability: the app asks for `postgreSqlDatabases` and the environment decides how to satisfy it.
- **Azure capabilities swapped where it matters.** Teams that go to Azure should see MQTT, workload identity, and AI change underneath while the app graph stays familiar.

Watch for the anti-approach: copying `app.bicep` and hard-coding environment-specific values into the copy. That produces a working deployment while defeating the entire point of the challenge.

## 4. Key Concepts & Teaching Points

- **The three layers.** Application definition (`radius/app.bicep`, app-team owned, unchanged), environment definition (`radius/local-env.bicep` / `radius/aks-env.bicep`, platform-team owned, chooses recipes and provider scope), and deployment parameters (image tag, password, identity client IDs, AI model, Azure scope). Most confusion in this challenge dissolves once a team can place a given change in the right layer.
- **Recipes are the portability seam.** The application asks for capabilities (`postgreSqlDatabases`, `mqttBrokers`, `workloadIdentities`, `aiModels`), not products. The environment binds each capability to a recipe. Not every capability has to differ per environment for the boundary to be real.
- **Capabilities that typically differ in Azure.** MQTT (in-cluster broker vs Azure Event Grid MQTT), workload identity (local/no-op vs Azure workload identity values), and AI (in-cluster vs Azure OpenAI). PostgreSQL stays containerized in both today.
- **`aiProvider=local` is a recipe pointer, not a location.** It means "use the environment-registered `aiModels` recipe." In the Azure environment that recipe is Azure OpenAI. Teams often misread `local` as "runs locally."
- **Naming is a distraction, not a lesson.** The sample artifact path uses the `rg-trading` group with `env-local-prod` and `env-azure-prod` environments. Challenge 2 may have introduced domain names such as `ws-azure-prod` or `rg-finance`. Either is fine as long as the team is consistent; do not let a naming debate consume the session.
- **Trade-offs worth surfacing.** Separate clusters vs one cluster with multiple environments; explicit deployment parameters vs environment-encoded difference; a portability demonstration vs full runtime parity (especially on the Azure MQTT path).

## 5. Common Pitfalls & Misconceptions

- **Editing `app.bicep` per environment.** The clearest sign a team has lost the thread. Redirect them to the environment or a parameter.
- **Hard-coding namespace, registry, or hostnames.** Defeats portability even if the deployment succeeds.
- **Deploying to the wrong place.** Switching `kubectl` context but forgetting to switch the `rad` workspace, or vice versa.
- **Skipping environment readiness.** Deploying the app before confirming the target environment has the recipes it needs, then debugging the app when the real gap is the environment.
- **Expecting a managed database in Azure.** Both environment files keep containerized PostgreSQL today; there is no managed Azure PostgreSQL recipe in the repository.
- **Assuming Azure MQTT works end to end.** The Event Grid MQTT recipe provisions the namespace endpoint only. Publish/subscribe also needs authenticated clients plus topic-space and permission-binding setup. A browser-reachable frontend can coexist with a broken messaging path.
- **Reading `aiProvider=local` literally.** It selects the recipe-backed AI resource; the environment decides whether that is Kaito or Azure OpenAI.
- **One control plane, one app name.** Deploying both environments to the same Radius control plane and group means the fixed application name `adaptive-apps` is the same resource, so the second deployment overwrites the first. Separate workspaces or groups avoid this.

## 6. Coaching Guidance (MANDATORY)

Lead with questions. Let teams sit with them. Do not answer on their behalf. Group the questions by where the team is in the challenge.

**While they choose and align environments**

- "How will you know, before you run anything, exactly where this deployment is going to land?"
- "What two independent settings decide the target, and how could they disagree with each other?"
- "What is your honest production/non-production story if you only have one cluster?"

**While they confirm readiness**

- "What has to already be true about an environment before this app can succeed there?"
- "If the app fails to connect to something, how will you decide whether that is an app problem or an environment problem?"

**While they deploy to the second environment**

- "Which file are you deploying, and is it the same one you used the first time?"
- "If you feel the urge to edit the application model, what layer does that change actually belong to?"
- "What is the smallest set of things that had to differ for Azure, and why those?"

**While they compare the two deployments**

- "What changed between the two deployments, and what stayed identical?"
- "Where is the platform-specific logic actually located?"
- "Why does the app still reach its database without a new connection string?"
- "What would make this app less portable than it is today?"
- "Who owns each file and each parameter you touched?"

**To stress-test understanding**

- "If leadership asked you to add a third environment tomorrow, what would the change set look like?"
- "Which of the differences you found are real portability boundaries, and which are gaps that belong in a recipe or in separate platform setup?"

## 7. Hint Strategy Guidance

The student guide already provides a progressive Hint 1/2/3 ladder per task. Mirror that escalation; do not jump to the strongest hint.

- **When to hint.** Only after a team has genuinely tried and can articulate what they attempted. A team that is productively debating does not need a hint.
- **How much to reveal.** Hint 1 reframes the problem as a question. Hint 2 names the relevant concept or layer. Hint 3 points at where to look without giving the command. If you find yourself about to dictate a command, stop and ask a question instead.
- **Avoid over-helping.** The most common coaching error here is rescuing a team from the "wrong place" mistake before they have felt it. Letting a team discover that they deployed to the wrong workspace is a better lesson than preventing it. Reserve direct intervention for hard blockers such as missing Azure credentials or a cluster that is genuinely down.
- **Calibrate to the timebox.** If a team is well short of time and still on environment setup, it is fair to be more direct about readiness so they reach the comparison, which is where the learning concentrates.

## 8. Validation Guidance

A team has met the objectives when they can demonstrate the following, in their own words and with their own evidence.

- The same `radius/app.bicep` file was used for both deployments (no fork, no per-environment edits).
- Each environment has registered recipes for the same portable resource types, and the team can point out which recipes match and which differ.
- The application graph or resource list is present in both environments and looks structurally the same.
- The frontend is reachable in both environments.
- The team can separate what changed into application-deployment parameters versus environment/platform changes, and assign ownership for each.
- The team can describe honestly where the Azure path is a demonstration rather than full parity (notably the Event Grid MQTT messaging path).

**Acceptable variations.** Two clusters or one cluster with two environments; difference carried by parameters or by the environment file; AI path enabled or skipped; MQTT left as an endpoint-only demonstration. Do not require a managed Azure database, and do not require end-to-end Event Grid MQTT messaging. The portability boundary, clearly explained, is the bar, not a maximal Azure build-out.

## 9. Optional Demo / Discussion Points

- **Boundary whiteboard.** Have a team draw the three layers and place each thing they changed onto the correct layer. Disagreements here are where the real understanding surfaces.
- **Cross-team compare.** If multiple teams chose different approaches (two clusters vs one cluster, parameter-carried vs environment-carried difference), have them present to each other and defend the trade-offs.
- **"Add a third environment" thought experiment.** Ask teams to estimate the change set for a hypothetical third environment without building it. A small, confident answer is a sign the boundary landed.
- **Portability gaps as a backlog.** Discuss which gaps (managed database, full Event Grid MQTT setup, identity-provider behavior) belong in recipes versus separate platform work, and who would own each. This connects the challenge to how the platform would actually evolve.

---

## Detailed Solution Walkthrough (Reference)

Sections 1-9 above are how to **coach** this challenge: facilitate, ask questions, and
let teams discover the portability boundary themselves. This walkthrough is the
validated deployment path so you have ground truth on hand - use it to confirm a team's
work, unblock a stuck team, or run the closing demo. Do **not** hand these commands to
teams up front; reach for them only after the hint ladder has been exhausted.

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
| `Radius.Resources/idProviders` | Keycloak OIDC provider recipe (registered by the local environment; `app.bicep` wires OIDC login through its `oidc*` parameters and does not instantiate this resource) |
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
kubectl get pods -n env-local-prod
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

The second environment should already have a Kubernetes cluster and Radius control plane from Challenge 1 and Challenge 2, and Challenge 4 already deploys `radius/aks-env.bicep` to register the Azure-backed recipes. In most cases this stage is a *confirmation* step, not a fresh build — the team usually only redeploys `aks-env.bicep` if the environment or recipes are missing, or to reconcile the environment name.

> **Naming note:** Challenge 4's sample deploys `aks-env.bicep` with `--environment trading` (the repository sample name), while the teaching standard from Challenge 2 is `env-azure-prod`. If the team followed Challenge 4's commands literally, their second environment may be named `trading` (workspace `aks-trading`). Either substitute that name wherever `env-azure-prod` appears below, or redeploy `aks-env.bicep` with `environmentName=env-azure-prod` to standardise on the teaching name. The goal of this challenge does not depend on the name — only on deploying the *same app* to whatever the second environment is called.

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

Deploy the Azure-backed environment definition **only if Challenge 4 did not already register these recipes** (or to reconcile the environment name to `env-azure-prod`):

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

`Radius.Resources/idProviders` is intentionally not registered in `aks-env.bicep`, and `app.bicep` does not instantiate an idProviders resource in either environment. The application's OIDC login is driven by its `oidc*` deployment parameters, so identity-provider behavior is configured at the application/parameter layer rather than carried by an environment recipe. Registering the recipe locally but not in Azure is a valid environment decision, not a portability failure.

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
    --subscription <subscription-id> \
    --resource-group <resource-group> \
    --output table
```

Expose the frontend from the second environment. If the Stage 2 expose session is still running on local port 3000, stop it first or pick a different local port so the two tunnels do not collide:

```bash
rad resource expose Applications.Core/containers frontend \
    -a adaptive-apps \
    --port 3001 \
    --remote-port 3000
```

Open `http://localhost:3001` and verify the deployment is reachable. If the second environment uses Azure Event Grid MQTT without topic spaces, permission bindings, and client authorization configured, browser access can work while MQTT-backed flows fail; use that as a coaching moment about which portability gaps belong in recipes versus separate platform setup.

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

### Validation checklist

At the end of this challenge, teams should be able to demonstrate:

- The same `radius/app.bicep` file was used for both deployments.
- Each environment registers recipes for the resource types the deployed app actually uses (PostgreSQL, MQTT, workload identities, plus AI/governance/guardrails when enabled); teams can identify intentional differences, such as `idProviders` being registered locally but not in Azure.
- The application deploys and is reachable in both environments; full runtime parity on the Azure MQTT path requires the Event Grid topic-space and permission setup called out above.
- The backing infrastructure can differ by environment without changing the application resource declarations; in the current repo, MQTT, workload identity, and AI show the clearest differences.
- The team can explain which changes were environment/platform changes and which were application deployment parameters.

### Common Issues

**Recipes are missing in the second environment**

Run `rad recipe list --environment env-azure-prod` in the second workspace. If the list is empty or only partially populated, redeploy the correct environment Bicep (`radius/local-env.bicep` or `radius/aks-env.bicep`) before deploying the app.

**The app deployed to the wrong cluster**

Run `rad workspace list` and `kubectl config current-context`. The `rad` workspace chooses the Radius control plane; the Kubernetes context is used when creating or updating that workspace. Teams often switch `kubectl` context but forget to switch `rad` workspace.

**Azure recipe deployment fails with authorization errors**

Confirm `rad credential register azure` was run against the second workspace and that the service principal has permission on the Azure resource group passed to `aks-env.bicep`.

**Azure MQTT auth or publish/subscribe fails**

Confirm AKS workload identity is enabled, the backend and frontend managed identity client IDs were passed to `app.bicep`, and the Event Grid namespace has the topic-space and permission-binding configuration required for those identities. The current `mqtt-azure-event-grid` recipe creates the namespace endpoint; it does not complete all MQTT authorization setup.

**The second deployment overwrote the first**

If both deployments target the same Radius control plane and group, the fixed application name `adaptive-apps` represents the same Radius application resource. Use separate workspaces/control planes for the cleanest environment split, or a separate Radius group per environment (`rg-trading` is itself a Radius group, so add a second group rather than nesting one inside it) if the team is simulating multiple environments on one control plane.

**AI works locally but fails in Azure**

Check that the Azure environment recipe for `Radius.Resources/aiModels` is registered and that the requested `aiModel` value is available in the target Azure OpenAI region. The app still passes `aiProvider=local` because that tells `app.bicep` to use the recipe-backed `aiModels` resource; the environment decides whether the recipe produces Kaito or Azure OpenAI.
