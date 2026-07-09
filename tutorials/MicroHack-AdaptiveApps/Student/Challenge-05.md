# Challenge 05 - Port the App Across Environments

[< Previous Challenge](./Challenge-04.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-06.md)

## Challenge Metadata

- Difficulty Level: Intermediate to Advanced
- Estimated Time: 45-75 minutes
- Target Audience: Platform and application engineers collaborating on multi-environment deployments
- Prerequisites:
	- Completion of [Challenge 01](./Challenge-01.md) through [Challenge 04](./Challenge-04.md)
	- At least two target environments (or two logical Radius targets) prepared and accessible
	- Radius workspaces, environments, groups, resource types, and recipes already registered
	- Access to the application model and environment artifacts (`radius/app.bicep`, environment bootstrap files)
- Learning Objectives:
	- Demonstrate application portability by deploying the same app model across environments
	- Identify which deployment concerns belong to application teams vs platform teams
	- Validate that environment recipe mappings drive implementation differences without app-model changes
	- Produce evidence and narrative that distinguish portability proof from full runtime parity

## Scenario

Executive stakeholders want the trading application expanded from a single environment to a second environment for resilience and operational flexibility. They expect the move to be fast because the team has invested in platform abstractions.

Your team must now prove that this expectation is realistic: the same application definition should deploy across environments while environment-specific behavior is controlled by recipe mappings and deployment target configuration.

This matters to both engineering and business. If portability requires app-model rewrites, release velocity drops and environment expansion becomes expensive. If portability works, teams can scale delivery without duplicating application logic.

## Challenge Goals

1. Deploy the same application model to two environments with only target and parameter differences.
2. Validate and explain how recipe/environment mappings change implementation while preserving app intent.
3. Present a clear portability evidence set and ownership model for application vs platform responsibilities.

## Requirements and Constraints

- The same application model must be used for both deployments.
- Environment readiness is mandatory: each target must have recipe mappings for required resource types.
- Differences between deployments must be limited to target selection and environment-specific parameters.
- Identity and access controls must follow the target environment security model; do not bypass identity requirements for convenience.
- If your second target is a workshop stand-in (for example two AKS clusters), state that limitation explicitly when presenting results.
- Treat messaging/runtime deep parity as separate from initial portability proof unless your team intentionally completes full platform setup.

## Tasks

### Task 1 - Define the Portability Plan

As a team, choose two deployment targets and define how you will prevent target drift (workspace, group, environment, and cluster context). Agree on what evidence will prove portability.

### Task 2 - Validate Environment Readiness

Assess both environments for recipe and capability readiness before deployment. Confirm that required resource types can be satisfied in each target.

### Task 3 - Deploy the Same Application Model to Both Targets

Execute deployments to each environment using the same application file, applying only environment-appropriate parameter values and security inputs.

### Task 4 - Compare Outcomes and Explain Ownership

Analyze results side by side and explain what changed, what stayed invariant, and which team owns each class of change.

## Success Criteria

You are done when all of the following are true:

- The same application model artifact is used for both target deployments.
- Both environments show successful app deployment evidence through Radius CLI/dashboard validation.
- Registered recipe mappings are visible for both environments and can be compared.
- The team can explain implementation differences (for example database or messaging backends) without app-model edits.
- The team can articulate portability scope and any remaining runtime parity gaps.
- A clear ownership split is documented between app-model responsibilities and platform/environment responsibilities.

## Hints (Progressive Disclosure)

### Task 1 Hints

- Hint 1: Most portability failures are targeting mistakes, not modeling mistakes.
- Hint 2: Treat workspace/group/environment selection as explicit deployment inputs, not assumptions.
- Hint 3: Build a quick pre-deploy checklist and run it before each environment deployment.

### Task 2 Hints

- Hint 1: A target is not ready just because Radius is installed.
- Hint 2: Check whether each required `Radius.Resources/*` capability has an environment recipe mapping.
- Hint 3: If a mapping is missing, fix the environment layer rather than modifying the app model.

### Task 3 Hints

- Hint 1: Keep the application model path identical across both deployments.
- Hint 2: Let parameterization carry environment specifics such as identity and endpoint choices.
- Hint 3: If you need code edits to deploy in the second environment, re-evaluate whether that change belongs in platform configuration.

### Task 4 Hints

- Hint 1: Compare both the app graph and backing resource behavior.
- Hint 2: Capture differences in two categories: deployment parameters and environment/recipe implementation.
- Hint 3: Include an honest statement about what was proven (portability) and what still needs extra setup (full behavior parity).

## Learning Resources

- Radius deployment model:
	- [Deploy applications with Radius](https://docs.radapp.io/guides/deploy-apps/)
	- [Radius environments overview](https://docs.radapp.io/guides/deploy-apps/environments/overview/)
	- [Radius workspaces overview](https://docs.radapp.io/guides/operations/workspaces/overview/)
- Extensibility and portability:
	- [Radius recipes overview](https://docs.radapp.io/guides/recipes/overview/)
- Azure capabilities referenced in this challenge:
	- [AKS workload identity overview](https://learn.microsoft.com/azure/aks/workload-identity-overview)
	- [Azure Event Grid MQTT overview](https://learn.microsoft.com/azure/event-grid/mqtt-overview)

## Optional Stretch

If you finish early, deploy to a third target environment and produce a short portability playbook that another team can follow without editing the application model.
