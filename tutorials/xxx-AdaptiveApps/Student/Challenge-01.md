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

## Description

As a team, set up and validate at least one target environment so that the following are true. If your team will complete the portability challenge during this event, you may optionally prepare a second target environment now:

- A Kubernetes cluster is available and the current `kubectl` context points at it from every team member's workstation. Any CNCF-conformant cluster works (AKS, kind, k3d, Arc-enabled Kubernetes, or Azure Local). AKS with OIDC issuer and workload identity enabled is strongly recommended for later challenges.
- An **Azure Container Registry (ACR)** exists and is attached to the cluster so it can pull images without a pull secret.
- An **Azure Key Vault** exists with RBAC authorization enabled and the cluster's identity has been granted appropriate data-plane access.
- An **Azure Storage account** exists and the cluster's identity has been granted `Storage Blob Data Contributor`.
- All role assignments are in place and `kubectl get nodes` shows a healthy cluster.

Treat this as a *platform engineering* exercise. The goal is a clean, verifiable environment that the rest of the team can build on.

> **NOTE:** Do not install Radius yet — that is Challenge 2. Focus on the cluster and supporting infrastructure first.

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

- [Azure Kubernetes Service documentation](https://learn.microsoft.com/azure/aks/) — quickstarts, concepts, and how-to guides for AKS.
- [Create an AKS cluster](https://learn.microsoft.com/azure/aks/learn/quick-kubernetes-deploy-cli) — CLI walkthrough for provisioning AKS with OIDC and workload identity.
- [Azure Container Registry overview](https://learn.microsoft.com/azure/container-registry/container-registry-intro) — what ACR is and how to attach it to AKS.
- [Azure Key Vault overview](https://learn.microsoft.com/azure/key-vault/general/overview) — concepts and RBAC authorization model.
- [Kubernetes: Install and Set Up kubectl](https://kubernetes.io/docs/tasks/tools/) — if you still need to configure cluster access on your workstation.
- [Arc-enabled Kubernetes overview](https://learn.microsoft.com/azure/azure-arc/kubernetes/overview) — if your team is targeting an Arc-enabled or Azure Local environment.

## Tips

- OIDC issuer and workload identity must be enabled at cluster *creation* time on AKS — they cannot be easily added retroactively. Plan ahead.
- The account used to create the cluster needs sufficient Azure RBAC permissions (Contributor on the resource group at minimum).
- If AKS provisioning seems to hang, check `az aks show` for provisioning state rather than retrying the create command.
- Start with a single environment (AKS) and only add a second environment (Arc, Azure Local) if time allows.
- If no Azure Local, Arc-enabled, k3d, or other Kubernetes target is available, your coach may optionally ask you to use a second AKS cluster as a workshop stand-in for the local/edge environment. This is not the same as Azure Local, but it gives Challenge 5 two real Radius control planes and environments to compare.

## Advanced Challenges (Optional)

Finished early? Try one or more of the following:

- Set up a **second** target environment (for example kind locally, an Arc-enabled cluster, Azure Local, or a second AKS cluster used as a workshop stand-in) so that Challenge 5 (Port the App Across Environments) has two environments to demonstrate portability.
- Write a short runbook explaining how to **tear down and recreate** the cluster cleanly, including the ACR, Key Vault, and Storage dependencies.
