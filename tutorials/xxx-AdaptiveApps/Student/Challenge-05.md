# Challenge 05 - Port the App Across Environments

[< Previous Challenge](./Challenge-04.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-06.md)

## Pre-requisites

- Completion of [Challenge 01](./Challenge-01.md): at least one healthy Kubernetes environment is available. A second environment is strongly recommended for this challenge.
- Completion of [Challenge 02](./Challenge-02.md): Radius is installed, and your workstation has a workspace, environment, and resource group configured.
- Completion of [Challenge 03](./Challenge-03.md): the portable `Radius.Resources/*` resource types have been registered.
- Completion of [Challenge 04](./Challenge-04.md): recipes are registered for at least one environment.
- Access to the repository's Radius assets, including `radius/app.bicep`, `radius/local-env.bicep`, and `radius/aks-env.bicep`.

## Introduction

In the previous challenges you built the platform foundation for portability. Resource types define the application-facing contract, and recipes tell each environment how to satisfy that contract.

In this challenge you prove that the contract works by deploying the same application model to more than one environment. The application should ask for capabilities such as a PostgreSQL database, an MQTT broker, workload identity, and optionally an AI model. The environment should decide whether those capabilities are backed by in-cluster services, Azure services, or another implementation.

The goal is not simply to run a deployment command twice. The goal is to show that the application definition stays portable while the platform-specific behavior moves into the environment and recipe layer.

## Description

Your team has been asked to port the Adaptive Apps trading application from one environment to another with minimal configuration changes.

Use the Radius application model in `radius/app.bicep` and deploy it to an environment that already has recipes registered from Challenge 4. Then deploy the same application model to a second environment. Do not copy or fork the application Bicep file just to change environment-specific settings.

As a team, complete the challenge so that the following are true:

- You can identify the first and second Radius workspaces, environments, and resource groups that you are targeting.
- You can show that the relevant `Radius.Resources/*` recipes are registered in each environment.
- You can deploy `radius/app.bicep` to the first environment and verify that the application resources are created.
- You can deploy the same `radius/app.bicep` file to a second environment by changing only deployment target and parameter values.
- You can compare the two environments and explain which backing implementations changed.
- You can explain why the application model did not need to hard-code database hostnames, MQTT endpoints, identity details, or AI endpoints.

If your second environment is AKS/Azure-backed, expect some capabilities to behave differently:

- The current repository keeps PostgreSQL on the Kubernetes recipe in both local and AKS environment files.
- The AKS/Azure environment can switch MQTT to Azure Event Grid MQTT, workload identity to Azure workload identity values, and AI to Azure OpenAI.
- The Azure Event Grid MQTT recipe provisions the namespace endpoint. Full publish/subscribe behavior also requires authenticated clients plus topic-space and permission-binding setup.

## Success Criteria

To complete this challenge successfully, you should be able to:

- Show the same `radius/app.bicep` file being used for both deployments.
- Show `rad recipe list` output for both environments and identify which recipes are the same and which are different.
- Show the Radius application graph or resource list for the application in both environments.
- Verify that the application frontend is reachable in both environments.
- Demonstrate that changing workspaces, environments, resource groups, and parameters is enough to move the app model across environments.
- Explain which concerns belong in the application definition and which belong in environment or recipe definitions.

## Learning Resources

- [Radius applications](https://docs.radapp.io/guides/deploy-apps/) - how Radius models applications, containers, connections, and environments.
- [Radius environments overview](https://docs.radapp.io/guides/deploy-apps/environments/overview/) - how environments provide deployment context and recipe configuration.
- [Radius recipes overview](https://docs.radapp.io/guides/recipes/overview/) - how recipes turn portable resource types into concrete infrastructure.
- [Radius workspaces](https://docs.radapp.io/guides/operations/workspaces/overview/) - how the `rad` CLI targets a specific Radius control plane.
- [Azure Event Grid MQTT broker support](https://learn.microsoft.com/azure/event-grid/mqtt-overview) - background on Azure Event Grid MQTT namespaces, clients, topic spaces, and permissions.
- [AKS workload identity](https://learn.microsoft.com/azure/aks/workload-identity-overview) - background on Kubernetes service account federation to Microsoft Entra workload identities.

## Tips

- Keep `radius/app.bicep` unchanged unless your coach explicitly asks you to investigate the application model. Portability is the thing you are proving.
- Use consistent names when switching workspaces and environments. It is easy to deploy to the wrong Radius control plane if your `kubectl` context and `rad workspace` are not aligned.
- If your team used the sample artifact path, you may see names such as `adaptive` for the Radius group and `trading` for the environment/namespace. If your team used the domain names from Challenge 2, substitute your own group and environment names consistently.
- Recipe output is the bridge between the platform and the application. When something cannot connect, inspect the registered recipe and the resource outputs before changing the application code.
- If you enable AI through the recipe-backed path, the application still uses the same `aiProvider=local` pattern. The environment decides whether that recipe produces a local/Kaito endpoint or Azure OpenAI.
- If your Azure MQTT-backed deployment is reachable but publish/subscribe does not work, check Event Grid MQTT topic spaces, permission bindings, and workload identity configuration.

## Advanced Challenges (Optional)

Finished early? Try one or more of the following:

- Deploy the same application to a third environment and document exactly which values changed.
- Replace one recipe in a non-production environment with a different implementation and show that `radius/app.bicep` does not change.
- Enable the optional AI path in both environments and compare the local/Kaito and Azure OpenAI-backed deployments.
- Create a short portability runbook that tells another team how to move the application between environments without editing application Bicep.
