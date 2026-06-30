# Challenge 01 - Prepare the Platforms

[< Previous Challenge](./Challenge-00.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-02.md)

## Challenge Metadata

- Difficulty Level: Intermediate
- Estimated Time: 30-60 minutes
- Target Audience: Builders and platform engineers working as a hack team
- Prerequisites:
	- Completion of [Challenge 00](./Challenge-00.md)
	- Access to an Azure subscription (for AKS/Arc/Azure Local paths)
	- Working local tools: Azure CLI, `kubectl`, `helm`, and `rad`
- Learning Objectives:
	- Select and prepare an environment strategy for the team
	- Validate Kubernetes platform readiness before Radius installation
	- Install and verify the baseline adaptive-apps portfolio release
	- Produce evidence that the team can safely continue to Challenge 02

## Scenario

Your team has been asked to launch a trading solution demo in a limited workshop window. Product and engineering leaders need confidence that the platform is stable before any Radius resources are created.

You are the platform squad for the team. Your responsibility is to stand up at least one target runtime environment, confirm shared access across teammates, and install the baseline portfolio chart that all later challenges rely on.

If your platform setup is inconsistent, every downstream challenge becomes harder to debug. A clean and validated platform now reduces delivery risk for the rest of the hack.

<<<<<<< Updated upstream
As a team, set up and validate at least one target environment so that the following are true. If your team will complete the portability challenge during this event, you may optionally prepare a second target environment now:
=======
## Challenge Goals
>>>>>>> Stashed changes

1. Prepare and validate at least one Kubernetes target environment that the full team can use.
2. Install the adaptive-apps portfolio baseline in a namespace/release strategy that matches your selected environment.
3. Capture and share validation evidence proving the platform is ready for Radius in Challenge 02.

## Requirements and Constraints

- Work as a team and use a shared environment decision (single primary cluster for all members).
- You may choose any supported environment path:
	- AKS
	- Local k3s (k3d)
	- Arc-enabled Kubernetes
	- Azure Local
- Radius installation is out of scope in this challenge.
- The portfolio deployment must match the environment capabilities (for example, Istio integration behavior may differ by platform).
- Keep your setup reproducible: another teammate must be able to validate the same environment state.

## Tasks

### Task 1 - Select a Platform Strategy

Design a platform approach for this challenge window. Decide which environment to use first, how teammates will share access, and what evidence you will collect to prove readiness.

### Task 2 - Prepare the Environment

Implement your selected environment path and configure team access so everyone can query the same cluster context. Validate that the cluster is healthy enough to host the portfolio baseline.

### Task 3 - Install the Portfolio Baseline

Deploy the adaptive-apps chart using a profile that matches your chosen environment. Ensure namespace/release conventions are clear and repeatable for the team.

### Task 4 - Validate Readiness for Next Challenge

Demonstrate that the release is visible and workloads are starting as expected. Document any environment-specific decisions and known caveats before moving on.

## Success Criteria

You are done when all of the following are true:

- The team can show a healthy Kubernetes cluster and consistent access from member workstations.
- The adaptive-apps portfolio release is installed and discoverable through Helm/Kubernetes validation commands.
- The deployed resources align with the chosen environment profile and expected behavior.
- The team can explain why the chosen platform path is appropriate for the remaining challenges.
- The team explicitly confirms Radius installation has not yet started.

## Hints (Progressive Disclosure)

### Task 1 Hints

- Hint 1: Favor one primary cluster for the team before considering multi-environment expansion.
- Hint 2: AKS is usually the fastest path for workshop consistency when Azure services are involved later.
- Hint 3: Decide your evidence format up front (terminal outputs, brief run notes, and ownership of checks).

### Task 2 Hints

- Hint 1: Use the environment-specific preparation guide as your implementation source of truth.
- Hint 2: If provisioning appears stalled, verify current state before re-running create operations.
- Hint 3: For AKS, identity-related flags are easiest to get right during initial cluster creation.

### Task 3 Hints

- Hint 1: Pick a portfolio profile intentionally (`core`, `core-ai`, `ent`, `ent-ai`, `min-ai`) based on your environment goals.
- Hint 2: Some AKS configurations use existing managed Istio instead of installing Istio from the chart.
- Hint 3: Keep release name and namespace simple and aligned so teammates can troubleshoot quickly.

### Task 4 Hints

- Hint 1: Validate both Helm release visibility and pod state before declaring success.
- Hint 2: A partially healthy deployment can still reveal useful blockers for the team to resolve together.
- Hint 3: Record caveats now so Challenge 02 setup time is not consumed by rediscovery.

## Learning Resources

- Environment preparation guides:
	- [Prepare AKS](../../common/prepare-aks.md)
	- [Prepare local k3s (k3d)](../../common/prepare-k3s.md)
	- [Prepare Arc-enabled Kubernetes](../../common/prepare-arc.md)
	- [Prepare Azure Local](../../common/prepare-azure-local.md)
- Portfolio install reference:
	- [Getting Started - Install the portfolio chart](../../getting-started/README.md)
- Product docs:
	- [Azure Kubernetes Service documentation](https://learn.microsoft.com/azure/aks/)
	- [Helm documentation](https://helm.sh/docs/)
	- [Kubernetes kubectl setup](https://kubernetes.io/docs/tasks/tools/)

## Optional Stretch

<<<<<<< Updated upstream
- OIDC issuer and workload identity must be enabled at cluster *creation* time on AKS — they cannot be easily added retroactively. Plan ahead.
- The account used to create the cluster needs sufficient Azure RBAC permissions (Contributor on the resource group at minimum).
- If AKS provisioning seems to hang, check `az aks show` for provisioning state rather than retrying the create command.
- Start with a single environment (AKS) and only add a second environment (Arc, Azure Local) if time allows.
- If no Azure Local, Arc-enabled, k3d, or other Kubernetes target is available, your coach may optionally ask you to use a second AKS cluster as a workshop stand-in for the local/edge environment. This is not the same as Azure Local, but it gives Challenge 5 two real Radius control planes and environments to compare.

## Advanced Challenges (Optional)

Finished early? Try one or more of the following:

- Set up a **second** target environment (for example kind locally, an Arc-enabled cluster, Azure Local, or a second AKS cluster used as a workshop stand-in) so that Challenge 5 (Port the App Across Environments) has two environments to demonstrate portability.
- Write a short runbook explaining how to **tear down and recreate** the cluster cleanly, including the ACR, Key Vault, and Storage dependencies.
=======
If you finish early, prepare a second environment and compare the trade-offs your team would face in Challenge 05 when demonstrating portability.
>>>>>>> Stashed changes
