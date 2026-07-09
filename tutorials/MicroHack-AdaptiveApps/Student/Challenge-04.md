# Challenge 04 - Build the Platform Abstractions with Recipes

[< Previous Challenge](./Challenge-03.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-05.md)

## Challenge Metadata

- Difficulty Level: Intermediate to Advanced
- Estimated Time: 60-90 minutes
- Target Audience: Platform engineers implementing environment-specific infrastructure behind portable application contracts
- Prerequisites:
	- Completion of [Challenge 03](./Challenge-03.md)
	- Registered resource types available in your Radius environment(s)
	- Access to Radius CLI, Radius dashboard, Azure CLI, and a container registry for publishing templates
	- Team agreement on environment naming and recipe ownership model
- Learning Objectives:
	- Implement a recipe that maps a portable type contract to a concrete infrastructure backend
	- Use Radius recipe context and outputs to produce predictable values and secure secrets
	- Publish and register recipes so environments can resolve implementations automatically
	- Compare local and Azure environment recipe strategies for the same portable resource model

## Scenario

Your organization has approved the portable resource contracts from the previous challenge, but application teams still cannot deploy real dependencies. Platform leadership now needs your team to provide the implementation layer that fulfills those contracts per environment.

You must deliver recipes that translate portable resource requests into real infrastructure while keeping developers insulated from provider-specific details. The solution needs to be supportable by platform operations and consistent across team members.

This is a high-impact milestone: if recipe mappings are incorrect, downstream application deployments will fail or leak infrastructure complexity back into application code.

## Challenge Goals

1. Build and register at least one custom recipe that implements a previously defined resource type contract.
2. Onboard a complete environment recipe set for the adaptive-apps platform model.
3. Validate that registered recipes expose the expected outputs/secrets and are discoverable for later deployment stages.

## Requirements and Constraints

- Maintain strict separation between contract (resource type) and implementation (recipe).
- Recipe outputs must align with the corresponding type schema, including secure handling for secrets.
- Keep recipe registration environment-aware; different environments may legitimately use different backends.
- Ensure your implementation is team-repeatable and can be validated without ad hoc manual assumptions.
- Scope this challenge to recipe onboarding only; full app deployment verification is handled later.

## Tasks

### Task 1 - Design the Recipe Implementation Strategy

Define how your team will implement one custom recipe for a database-style resource type, including backend choice, configuration boundaries, and output/secret mapping expectations.

### Task 2 - Publish and Register a Custom Recipe

Implement your recipe artifact, publish it to the team registry path, and register it against the target resource type in the intended environment.

### Task 3 - Onboard the Environment Recipe Baseline

Apply the repository-provided environment recipe registration model and ensure your chosen environment has the expected portable resource implementations available.

### Task 4 - Validate Operational Readiness

Inspect recipe registrations through CLI and dashboard views, then confirm your team can explain which implementation is selected per resource type and environment.

## Success Criteria

You are done when all of the following are true:

- A custom recipe is registered for a target resource type and tied to the intended environment.
- Recipe registration state is visible and verifiable using both CLI and dashboard workflows.
- The environment shows a complete, usable recipe baseline for upcoming application deployment challenges.
- Your team can explain how recipe outputs map to type outputs, including secure secret handling.
- The team can describe how the same resource type could map to different implementations across environments.

## Hints (Progressive Disclosure)

### Task 1 Hints

- Hint 1: Start from the type contract and design the recipe interface to satisfy it exactly.
- Hint 2: Keep implementation thin where possible by reusing curated infrastructure modules.
- Hint 3: Treat output and secret mapping as contract obligations, not optional convenience data.

### Task 2 Hints

- Hint 1: Publishing and registration are separate concerns; validate each step independently.
- Hint 2: Use deterministic naming and versioning so teammates can reproduce results.
- Hint 3: If registration looks correct but behavior is unexpected, re-check environment targeting and template reference.

### Task 3 Hints

- Hint 1: Environment bootstrap artifacts can register multiple recipes consistently in one operation.
- Hint 2: Compare local and Azure registration outcomes to understand portability boundaries.
- Hint 3: Some resource types may intentionally share the same implementation across environments.

### Task 4 Hints

- Hint 1: Validate both existence and semantic fit: a registered recipe is useful only if it matches contract expectations.
- Hint 2: Ask the team to explain selection logic for recipe names and environment scope.
- Hint 3: Capture a short readiness checklist now to reduce friction in the application deployment challenge.

## Learning Resources

- Radius concepts and extensibility:
	- [Radius concepts overview](https://docs.radapp.io/concepts/)
	- [Radius recipes overview](https://docs.radapp.io/guides/extensibility/recipes/)
	- [Radius CLI reference](https://docs.radapp.io/reference/cli/)
- Infrastructure implementation patterns:
	- [Azure Verified Modules (AVM)](https://aka.ms/avm)
- Repository assets used in this challenge:
	- [AKS environment bootstrap](../../radius/aks-env.bicep)
	- [Local environment bootstrap](../../radius/local-env.bicep)
	- [Recipes folder](../../radius/recipes/)

## Optional Stretch

If you finish early, register an alternate recipe implementation for the same resource type and document when your team would choose one implementation over the other.
