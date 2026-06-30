# Challenge 01 - Prepare the Platforms

[< Previous Challenge](./Challenge-00.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-02.md)

## Pre-requisites

- Completion of [Challenge 00](./Challenge-00.md) and all tooling installed on your workstation (Azure CLI, `kubectl`, a code editor, etc.).
- An Azure subscription in which you have permission to create resources.

## Introduction

Before Radius can be installed and applications deployed, the target platform must be ready. In this challenge you play the role of the **platform engineer** on your team: your job is to provision and validate one or more Kubernetes environments and the supporting Azure resources (container registry, key vault, storage) that recipes in later challenges will depend on.

A well-prepared platform is the foundation for everything that follows. Decisions made here — which cluster type, which networking model, which identity approach — affect every later challenge. Take the time to get it right.

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

To complete this challenge successfully, you should be able to:

- Show that `kubectl get nodes` returns all nodes in `Ready` state on each team member's workstation.
- Show that the ACR, Key Vault, and Storage account exist and that the appropriate role assignments are in place for the cluster identity.
- Demonstrate that the cluster can pull a test image from the ACR without a pull secret.
- Describe the cluster type chosen, the networking model, and why the team made those decisions.

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
