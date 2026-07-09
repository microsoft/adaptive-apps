# Challenge 03 - Build the Platform Abstractions

[< Previous Challenge](./Challenge-02.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-04.md)

## Challenge Metadata

- Difficulty Level: Intermediate
- Estimated Time: 45-75 minutes
- Target Audience: Platform engineers and developers collaborating on a shared Radius platform
- Prerequisites:
	- Completion of [Challenge 02](./Challenge-02.md)
	- Working Radius control plane, workspace, environment, and resource group setup
	- Access to the Radius dashboard and `rad` CLI
	- Team agreement on naming conventions and environment scope
- Learning Objectives:
	- Define a custom Radius resource type as a reusable platform contract
	- Distinguish resource types from recipes and explain the responsibilities of each
	- Register and inspect a full set of platform resource types for later application deployment
	- Evaluate type schemas for inputs, outputs, and secret handling

## Scenario

Your platform team is onboarding application developers who need a stable self-service contract for infrastructure dependencies. Leadership wants teams to request resources in a consistent way without coupling application code to environment-specific implementations.

Before recipes are introduced, your team must define the abstraction layer developers will use. That means creating and reviewing resource type schemas that capture required inputs, expected outputs, and sensitive data handling.

This matters because every later deployment depends on these contracts. If the abstraction is unclear, application teams will hard-code assumptions and portability across environments will break.

## Challenge Goals

1. Create and register at least one custom resource type that demonstrates a valid platform contract.
2. Import and validate the shared resource type catalog required by the adaptive-apps scenario.
3. Demonstrate team understanding of how resource type schemas support portability and secure connections.

## Requirements and Constraints

- Keep this challenge focused on type contracts only; recipe implementation is not part of scope yet.
- Ensure every type clearly separates developer-provided inputs from platform-generated outputs.
- Sensitive values must be modeled for secure handling and must not be treated as ordinary plain-text outputs.
- Use schemas that can support multiple environment implementations without changing application intent.
- Work as a team and produce a shared explanation of your schema decisions.

## Tasks

### Task 1 - Design a Portable Resource Contract

Design a custom resource type for a database dependency that application teams can consume consistently. Define required inputs, optional tuning properties, and expected outputs in a way that can work across multiple environments.

### Task 2 - Register and Validate the Custom Type

Register your custom type in Radius and confirm it is discoverable and inspectable through your preferred validation methods. Verify the schema expresses the contract your team intended.

### Task 3 - Onboard the Shared Resource Type Catalog

Import the repository-provided resource types needed for upcoming challenges. Evaluate how each type models developer inputs, platform outputs, and secrets.

### Task 4 - Explain the Abstraction Model to the Team

Prepare a short architecture narrative showing how application code can remain stable while implementations vary by environment. Include the relationship between resource type, recipe, environment, and connection outputs.

## Success Criteria

You are done when all of the following are true:

- A custom resource type is registered and visible, with a schema your team can explain.
- The shared resource type catalog is registered and available for inspection.
- Your team can clearly differentiate resource types (contract) from recipes (implementation).
- Your team can demonstrate where standard outputs and secrets are modeled in at least two types.
- The team can explain how the same application definition can remain portable across environments.

## Hints (Progressive Disclosure)

### Task 1 Hints

- Hint 1: Start by identifying what developers must provide versus what the platform should return.
- Hint 2: Treat contract design as an API design exercise for internal platform consumers.
- Hint 3: If a field would be environment-specific runtime data, consider modeling it as read-only output.

### Task 2 Hints

- Hint 1: Validate the type from both CLI and dashboard perspectives to catch schema misunderstandings.
- Hint 2: If the type appears but is hard to reason about, refine descriptions and required fields.
- Hint 3: Make sure your schema supports secure value handling patterns before moving on.

### Task 3 Hints

- Hint 1: Compare several imported types to spot common schema anchors and naming patterns.
- Hint 2: Focus on consistency of inputs/outputs, not implementation details.
- Hint 3: Review at least one AI-related type and one data-related type to test your understanding of variation.

### Task 4 Hints

- Hint 1: Use a simple contract-versus-implementation explanation that non-platform teammates can follow.
- Hint 2: Highlight that portability comes from stable contracts combined with environment-specific recipes.
- Hint 3: Include one concrete example of how connection data can be injected without hard-coded connection strings.

## Learning Resources

- Radius core concepts:
	- [Radius concepts overview](https://docs.radapp.io/concepts/)
	- [Resource types and recipes](https://docs.radapp.io/guides/extensibility/recipes/)
- Radius operations and tooling:
	- [Radius dashboard](https://docs.radapp.io/guides/tooling/dashboard/)
	- [Radius CLI reference](https://docs.radapp.io/reference/cli/)
- Repository assets for this challenge:
	- [Radius resource type catalog](../../radius/resource-types/types.yaml)
	- [Radius folder overview](../../radius/README.md)

## Optional Stretch

If you finish early, propose a new custom resource type your organization might need and justify its contract design choices, including expected outputs and secure secret handling.
