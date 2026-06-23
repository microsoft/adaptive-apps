# Challenge 05 - Port the App Across Environments - Coach's Guide

[< Previous Solution](./Solution-04.md) - **[Home](./README.md)** - [Next Solution >](./Solution-06.md)

> This is a facilitation guide. It deliberately contains no step-by-step solution or command sequence. Your job is to help teams reason their way to a working second deployment, not to hand them the commands. The concrete deployment flow lives in the student-facing assets and the earlier challenges; resist the urge to read it out.

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
