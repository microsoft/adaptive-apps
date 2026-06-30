# Challenge 02 - Deploy and Explore Radius

[< Previous Challenge](./Challenge-01.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-03.md)

## Challenge Metadata

- Difficulty Level: Intermediate
- Estimated Time: 30-45 minutes per environment
- Target Audience: Platform engineers and app teams operating shared Kubernetes environments
- Prerequisites:
	- Completion of [Challenge 01](./Challenge-01.md)
	- Access to at least one prepared Kubernetes environment
	- Working tools on each workstation: `kubectl`, `rad`, and optional `helm`
	- Permissions to install platform components on the target cluster(s)
- Learning Objectives:
	- Compare Radius control plane deployment models and justify a team decision
	- Establish a working Radius control plane and local team workspaces
	- Validate environment and resource-group readiness for upcoming application deployment
	- Build operational understanding of Radius components through dashboard exploration

## Scenario

Your organization is preparing a multi-environment trading platform rollout and needs a consistent application platform layer before development teams can ship workloads.

Platform leadership has asked your team to onboard Radius now, but with an architectural decision that will affect resilience, governance, and day-2 operations. The team must determine whether to centralize control or federate by site, then prove the chosen model is operationally ready.

This decision matters: an incorrect control-plane strategy can increase outage blast radius or slow cross-site delivery. Your outcome should balance reliability, operational simplicity, and workshop time constraints.

## Challenge Goals

1. Select and justify a Radius control-plane model suitable for your environment(s).
2. Make Radius operational for your team, including workspace, environment, and resource-group readiness.
3. Demonstrate understanding of the deployed Radius platform by validating components and exploring the dashboard.

## Requirements and Constraints

- Work as one team and agree on naming conventions for workspaces, environments, and resource groups.
- Support one or more target environments (AKS, k3s/k3d, Arc-enabled Kubernetes, Azure Local) based on your challenge strategy.
- Keep the setup repeatable so any teammate can target the same control plane safely.
- Treat this challenge as platform enablement only: recipe authoring and application deployment come later.
- If targeting multiple sites, your approach must account for connectivity assumptions and failure isolation.

## Tasks

### Task 1 - Decide the Control-Plane Strategy

Evaluate centralized versus federated control-plane approaches for your team scenario. Select one model and document why it best fits your operational and resilience needs.

### Task 2 - Enable Radius on the Chosen Environment(s)

Implement Radius installation and team access for your selected model. Ensure each participant can target the intended control plane from their own workstation.

### Task 3 - Configure Team Scope

Create and validate the workspace and environment boundaries your team will use in upcoming challenges. Confirm resource-group organization supports your domain/team structure.

### Task 4 - Explore and Explain the Platform

Use CLI and dashboard validation to inspect the deployed Radius components and hierarchy. Prepare a brief team explanation of how control plane, workspace, environment, and group scopes interact.

## Success Criteria

You are done when all of the following are true:

- The team can articulate why the chosen deployment model is appropriate for the scenario.
- Radius is healthy and observable in the target environment(s), with control-plane components available.
- Each team member can target the expected workspace and see the expected environment/group scope.
- The team can demonstrate dashboard access and identify key Radius platform components.
- The team confirms no application deployment work has started yet.

## Hints (Progressive Disclosure)

### Task 1 Hints

- Hint 1: Start from failure domains and connectivity assumptions, not just installation simplicity.
- Hint 2: Federated models improve site autonomy; centralized models can simplify governance.
- Hint 3: For multi-site or intermittently connected environments, evaluate whether a single shared control plane is acceptable risk.

### Task 2 Hints

- Hint 1: Validate the active Kubernetes context before any installation action.
- Hint 2: On shared clusters, one install operation can serve the whole team while each person configures local targeting.
- Hint 3: Initial startup can take several minutes; check runtime health before re-running install commands.

### Task 3 Hints

- Hint 1: Use a consistent naming pattern so scopes are easy to recognize in CLI output.
- Hint 2: Workspace scope is local to each workstation, while environments/groups are platform objects.
- Hint 3: Multi-site teams should avoid ambiguous names that hide which site is being targeted.

### Task 4 Hints

- Hint 1: Validate from both CLI and dashboard views; each reveals different operational details.
- Hint 2: Focus your explanation on component responsibilities and scope boundaries.
- Hint 3: A short architecture summary prepared now will accelerate Challenge 03 collaboration.

## Learning Resources

- Radius concepts and architecture:
	- [What is Radius?](https://docs.radapp.io/concepts/)
	- [Install Radius on Kubernetes](https://docs.radapp.io/guides/operations/kubernetes/install/)
- Team operations:
	- [Radius workspaces overview](https://docs.radapp.io/guides/operations/workspaces/overview/)
	- [Radius environments overview](https://docs.radapp.io/guides/deploy-apps/environments/overview/)
	- [Radius dashboard](https://docs.radapp.io/guides/tooling/dashboard/)
- Environment-specific workshop guides:
	- [Prepare Radius on AKS](../../common/prepareRadius-aks.md)
	- [Prepare Radius on k3s](../../common/prepareRadius-k3s.md)
	- [Prepare Radius on Arc](../../common/prepareRadius-arc.md)
	- [Prepare Radius on Azure Local](../../common/prepareRadius-azure-local.md)

## Optional Stretch

If you finish early, evaluate a second control-plane topology for comparison and document trade-offs in operability, resilience, and team workflow.
