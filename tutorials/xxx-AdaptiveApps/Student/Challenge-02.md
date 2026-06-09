# Challenge 02 - Deploy and Explore Radius

[< Previous Challenge](./Challenge-01.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-03.md)

## Pre-requisites

- Completion of [Challenge 01](./Challenge-01.md): a healthy Kubernetes cluster is available and `kubectl get nodes` shows all nodes `Ready`.
- ACR, Key Vault, and Storage account are provisioned and role assignments are in place.

## Introduction

[Radius](https://radapp.io) is an open-source, cloud-native application platform that lets developers describe their entire application — containers, databases, message brokers, identities, and the cloud resources they depend on — as a single, portable model. Platform engineers use Radius to define reusable *environments* and *recipes* that automatically provision the right infrastructure, apply organizational policy, and keep developers focused on their code rather than the cloud plumbing underneath.

In this challenge you install the **Radius control plane** onto the cluster you prepared in Challenge 1, configure the `rad` CLI on every team member's workstation to point at that control plane, and then explore the Radius application model before moving on to building abstractions.

## Description

Your team has been asked to get a working Radius installation ready and to understand its core concepts before authoring any recipes or deploying any applications.

As a team, deploy and explore Radius so that the following are true:

- The `rad` CLI is installed on each team member's workstation and `rad version` reports a valid CLI version.
- The Radius control plane is installed into the cluster (in its own namespace) and all of its pods are `Running` / `Ready`.
- A Radius **workspace** is configured on each workstation so that the `rad` CLI targets the shared control plane.
- A default Radius **environment** exists in the control plane, is listed as the active environment for your workspace, and has the Azure cloud provider registered against the subscription and resource group from Challenge 1.
- Your team can explain, in its own words, what the Radius control plane is, which components are running, and how the `rad` CLI, workspace, environments, and the application model relate to each other.
- Your team has opened the Radius **dashboard** and can navigate it to inspect the environment and resource groups.

> **NOTE:** Do not author recipes or deploy applications yet — that is Challenge 3 and onwards. The goal here is a verified Radius installation and a shared understanding of the application model.

## Success Criteria

To complete this challenge successfully, you should be able to:

- Run `rad version` on each workstation and see a valid CLI version and a matching control plane version.
- Show that `kubectl get pods -n radius-system` returns all Radius pods as `Running` / `Ready`.
- Show that `rad workspace list` displays a workspace pointing at your cluster, marked as current.
- Show that `rad env list` returns at least one environment, marked as the default.
- Open the Radius dashboard and point out the environment and any registered cloud providers.
- Describe the role of each major Radius component (UCP, applications-rp, controller, dashboard) and why it matters.

## Learning Resources

- [What is Radius?](https://docs.radapp.io/concepts/) — overview of Radius concepts, including the control plane, environments, and recipes.
- [Install the rad CLI](https://docs.radapp.io/installation/) — how to obtain and verify the Radius command-line tool on Windows, macOS, and Linux.
- [Install Radius on a Kubernetes cluster](https://docs.radapp.io/guides/operations/kubernetes/install/) — supported cluster types, required permissions, and installation options.
- [Radius workspaces](https://docs.radapp.io/guides/operations/workspaces/overview/) — what a workspace is and how it connects the `rad` CLI to a control plane.
- [Radius environments overview](https://docs.radapp.io/guides/deploy-apps/environments/overview/) — how environments relate to the control plane and why they matter for later challenges.
- [Radius dashboard](https://docs.radapp.io/guides/tooling/dashboard/) — how to open and use the built-in Radius UI.

## Tips

- The account that runs `rad install kubernetes` needs `cluster-admin` permissions on the cluster. Use `az aks get-credentials --admin` on AKS to get admin credentials if needed.
- Only **one** team member needs to run `rad install kubernetes` — it installs into the shared cluster. Every other team member only needs to configure their local workspace.
- If `rad install kubernetes` seems to hang, check `kubectl get pods -n radius-system` — image pulls on a fresh cluster can take a few minutes before everything becomes `Ready`.
- Run `rad init --full` as an interactive alternative to the individual `rad workspace create` / `rad group create` / `rad env create` commands if you prefer a guided setup.

## Advanced Challenges (Optional)

Finished early? Try one or more of the following:

- Explore the Radius dashboard (`rad dashboard`) and use it to inspect the workspace and environment visually.
- Write a short runbook for your team explaining how to **upgrade** or **uninstall** Radius cleanly, including what happens to existing environments and applications.
- Investigate the Radius CRDs installed in the cluster (`kubectl get crds | grep radapp.io`) and describe what each one represents.
