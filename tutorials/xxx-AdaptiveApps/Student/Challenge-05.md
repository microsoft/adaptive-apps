# Challenge 05 - Port the App Across Environments

[< Previous Challenge](./Challenge-04.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-06.md)

## Pre-requisites

- Completion of [Challenge 01](./Challenge-01.md): at least one healthy Kubernetes environment is available. A second environment is strongly recommended for this challenge.
- Completion of [Challenge 02](./Challenge-02.md): Radius is installed, and your workstation has a workspace, environment, and resource group configured.
- Completion of [Challenge 03](./Challenge-03.md): the portable `Radius.Resources/*` resource types have been registered.
- Completion of [Challenge 04](./Challenge-04.md): recipes are registered for at least one environment.
- Access to the repository's Radius assets, including `radius/app.bicep`, `radius/local-env.bicep`, and `radius/aks-env.bicep`.

**Difficulty:** Intermediate &nbsp;|&nbsp; **Estimated time:** 45-75 minutes &nbsp;|&nbsp; **Audience:** platform and application engineers

By the end of this challenge you should be able to:

- Explain the portability boundary between the application model and the environment/recipe layer.
- Deploy the same `radius/app.bicep` to two environments by changing only the deployment target and parameter values.
- Compare the two deployments and identify which backing implementations changed.
- Articulate which concerns belong to the application team and which belong to the platform team.

## Introduction

Adaptive Apps has been running the trading application in a single environment. The platform team built that environment on a local or Azure Local Kubernetes cluster, and the application has been stable. Now the business wants a second environment.

Leadership has asked for a production environment in Azure for resilience, scale, and proximity to managed services, while the existing environment stays in place. The expectation from leadership is simple to say and harder to prove: *"It is the same application, so standing it up somewhere else should not be a rebuild."*

This puts two groups in the room together:

- The **application team** owns the trading app. They do not want to maintain a separate copy of the application for every environment, and they do not want cloud-specific plumbing leaking into their code.
- The **platform team** owns where and how the app runs. They decide which environment backs each capability with an in-cluster service, an Azure service, or something else, and they are responsible for identity, networking, and provider scope.

In the previous challenges you built the foundation that makes this possible. Resource types define the application-facing contract, and recipes tell each environment how to satisfy that contract. This challenge is where that investment has to pay off. The application asks for capabilities such as a PostgreSQL database, an MQTT broker, workload identity, and optionally an AI model. The environment decides whether those capabilities are backed by in-cluster services, Azure services, or another implementation.

The goal is not to run a deployment command twice. The goal is to demonstrate that the application definition stays portable while the platform-specific behavior lives in the environment and recipe layer, and to be able to explain to leadership exactly what had to change to move to the second environment.

## Description

Your team has been asked to port the Adaptive Apps trading application to a second environment with minimal configuration changes, and to be ready to defend the word "minimal" when leadership asks what changed.

Use the Radius application model in `radius/app.bicep` and an environment that already has recipes registered from Challenge 4. Then bring the same application model to a second environment. Do not copy or fork the application Bicep file just to change environment-specific settings.

### Your goals

- Run the same application model in two environments without maintaining two copies of it.
- Keep every platform-specific decision in the environment and recipe layer, not in the application.
- Be able to explain, in business terms, what genuinely changed between the two environments and why.

### Requirements and constraints

- `radius/app.bicep` must stay unchanged. If you find yourself editing it to make an environment work, treat that as a signal to look at the environment or a parameter instead.
- Each target environment must have the relevant `Radius.Resources/*` recipes registered before the application is deployed.
- Only the deployment target (workspace, environment, resource group) and parameter values may differ between the two deployments.
- The Azure-backed path relies on workload identity for the backend and frontend workloads; plan for that rather than weakening security to get a green deployment.
- The current repository keeps PostgreSQL on the Kubernetes recipe in both the local and AKS environment files, so do not expect a managed database to appear automatically in Azure.
- The Azure Event Grid MQTT recipe provisions the namespace endpoint only. Full publish/subscribe behavior also requires authenticated clients plus topic-space and permission-binding setup, so treat end-to-end MQTT in Azure as out of scope unless your team completes that setup deliberately.

### Work through these as a team

- Decide which two environments you are targeting, and how you will keep your `kubectl` context and `rad` workspace aligned so you never deploy to the wrong place.
- Confirm what each environment already provides before you deploy anything, so a missing recipe is not discovered halfway through.
- Demonstrate the same application model reaching both environments by changing only the target and parameters.
- Investigate which capabilities are backed differently in the second environment, and trace where that difference is actually expressed.
- Form a clear answer to the question "what did we have to change, and who owned each of those changes?"

## Success Criteria

To complete this challenge successfully, you should be able to:

- Show the same `radius/app.bicep` file being used for both deployments.
- Show the registered recipes for both environments and identify which are the same and which are different.
- Show the Radius application graph or resource list for the application in both environments.
- Verify that the application frontend is reachable in both environments.
- Demonstrate that changing workspaces, environments, resource groups, and parameters is enough to move the app model across environments.
- Explain which concerns belong in the application definition and which belong in environment or recipe definitions.
- Describe honestly where the Azure path is a portability demonstration versus full runtime parity (for example, the Event Grid MQTT messaging path).

## Learning Resources

- [Radius applications](https://docs.radapp.io/guides/deploy-apps/) - how Radius models applications, containers, connections, and environments.
- [Radius environments overview](https://docs.radapp.io/guides/deploy-apps/environments/overview/) - how environments provide deployment context and recipe configuration.
- [Radius recipes overview](https://docs.radapp.io/guides/recipes/overview/) - how recipes turn portable resource types into concrete infrastructure.
- [Radius workspaces](https://docs.radapp.io/guides/operations/workspaces/overview/) - how the `rad` CLI targets a specific Radius control plane.
- [Azure Event Grid MQTT broker support](https://learn.microsoft.com/azure/event-grid/mqtt-overview) - background on Azure Event Grid MQTT namespaces, clients, topic spaces, and permissions.
- [AKS workload identity](https://learn.microsoft.com/azure/aks/workload-identity-overview) - background on Kubernetes service account federation to Microsoft Entra workload identities.

## Hints

Use these only when your team is stuck. Each task starts with a nudge and gets more specific. Try the earlier hint before opening the next one.

If your team used the sample artifact path, the Radius group is `rg-trading` and the environments are `env-local-prod` and `env-azure-prod`. If your team used different domain names from Challenge 2, substitute your own group and environment names consistently.

### Targeting the right environment

- *Hint 1:* The most common failure in this challenge is deploying to the place you did not mean to. What two independent settings decide where a deployment actually lands?
- *Hint 2:* The `rad` workspace selects the Radius control plane, while your `kubectl` context affects how that workspace was created or updated. They can drift apart without warning.
- *Hint 3:* Before each deployment, confirm both the active workspace/group and the cluster context, and make switching them a deliberate step rather than an assumption.

### Confirming each environment is ready

- *Hint 1:* A green deployment depends on something that was set up in an earlier challenge. What has to already exist in an environment before the app can land there?
- *Hint 2:* The application asks for capabilities through `Radius.Resources/*`; the environment must already map each of those to a recipe.
- *Hint 3:* Inspect the recipes registered for the target environment first. If a capability the app needs is missing, fix that in the environment before touching the deployment.

### Moving the same app model to both environments

- *Hint 1:* If you are tempted to edit `radius/app.bicep` to make the second environment work, stop and ask what layer that change really belongs to.
- *Hint 2:* The differences between environments are expressed as deployment parameters and as the recipes the environment selected, not as changes to the application resources.
- *Hint 3:* Keep the application file path identical for both deployments and let the target plus a small set of parameters carry the difference. The Azure path needs identity-related parameters that the local path does not.

### Comparing and explaining the difference

- *Hint 1:* "Minimal changes" is only convincing if you can point at exactly what changed. How will you capture that side by side?
- *Hint 2:* Compare the application graph and resource list in both environments; they should look familiar, while the backing infrastructure and recipe outputs differ.
- *Hint 3:* Build a short before/after comparison that separates application-deployment parameters from environment/platform changes, and name who owns each one.

### Understanding the Azure-backed capabilities

- *Hint 1:* Some capabilities look identical from the app's perspective but are satisfied very differently underneath. Which ones are most likely to differ in Azure?
- *Hint 2:* In the current repository PostgreSQL stays containerized in both environments, while MQTT, workload identity, and AI are the capabilities that change in Azure.
- *Hint 3:* If the Azure frontend loads but a messaging-dependent flow fails, look at the Event Grid MQTT topic spaces, permission bindings, and workload identity configuration rather than the application code.

## Advanced Challenges (Optional)

Finished early? Try one or more of the following:

- Deploy the same application to a third environment and document exactly which values changed.
- Replace one recipe in a non-production environment with a different implementation and show that `radius/app.bicep` does not change.
- Enable the optional AI path in both environments and compare the local/Kaito and Azure OpenAI-backed deployments.
- Create a short portability runbook that tells another team how to move the application between environments without editing application Bicep.
