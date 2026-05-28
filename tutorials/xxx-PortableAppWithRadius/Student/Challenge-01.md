# Challenge 01 - Install and Configure the Radius Control Plane

[< Previous Challenge](./Challenge-00.md) - **[Home](../README.md)** - [Next Challenge >](./Challenge-02.md)

## Pre-requisites

- Completion of [Challenge 00](./Challenge-00.md) and all tooling installed on your workstation (Azure CLI, `kubectl`, a code editor, etc.).
- An Azure subscription in which you have permission to create resources.
- A running Kubernetes cluster that you can reach from your workstation with `kubectl`. Any CNCF-conformant cluster is supported (for example AKS, kind, k3d, or K3s).

## Introduction

[Radius](https://radapp.io) is an open-source, cloud-native application platform that lets developers describe their entire application — containers, databases, message brokers, identities, and the cloud resources they depend on — as a single, portable model. Platform engineers use Radius to define reusable *environments* and *recipes* that automatically provision the right infrastructure, apply organizational policy, and keep developers focused on their code rather than the cloud plumbing underneath.

Before any of that is possible, somebody needs to stand up the **Radius control plane**. The control plane is the set of services that run inside a Kubernetes cluster and coordinate everything Radius does: it accepts deployments from the `rad` CLI, drives Bicep-based rendering, reconciles application resources, and talks to your cloud provider when a recipe provisions infrastructure. Getting this foundation right — the cluster, the CLI, the control plane components, and the initial workspace/environment — is what makes the rest of the hack possible.

In this challenge, you will play the role of the platform engineer on your team. Your job is to prepare a workstation and a Kubernetes cluster, install the Radius control plane into that cluster, and verify that everything is wired up correctly so that in later challenges you can author recipes, define environments, and deploy a portable application with Radius.

## Description

Your team has been asked to get a working Radius installation ready for the rest of the hack. By the end of this challenge, every team member should be able to talk to the same Radius control plane from their own workstation.

As a team, install and configure Radius so that the following are true:

- The `rad` CLI is installed on each team member's workstation and the installed version is reported correctly.
- A Kubernetes cluster is available and the current `kubectl` context points at it.
- The Radius control plane is installed into the cluster (in its own namespace) and all of its pods are healthy.
- A Radius **workspace** is configured locally on each workstation so that the `rad` CLI talks to the cluster you just prepared.
- A default Radius **environment** exists in the control plane and is listed as the active environment for your workspace. This environment will be reused and extended in the following challenges (recipes, applications, and deployments).
- Your team can describe, in its own words, what the Radius control plane is, which components were installed, and how the `rad` CLI, the workspace, and the environment relate to each other.

Treat this as a *platform setup* exercise rather than an application exercise — there is nothing to deploy yet. The goal is a clean, verifiable Radius installation that the rest of the team can build on.

> **NOTE:** Do not hand-edit cluster manifests to "make it work". Use the supported Radius installation path so your environment matches what future challenges expect.

## Success Criteria

To complete this challenge successfully, you should be able to:

- Verify that `rad version` runs on your workstation and reports a valid CLI and (once installed) control plane version.
- Verify that the Radius control plane pods are in a `Running` / `Ready` state in the Radius namespace of your Kubernetes cluster.
- Show that `rad workspace list` displays a workspace pointing at your cluster and that it is marked as the current workspace.
- Show that `rad env list` returns at least one environment and that it is selected as the default for your workspace.
- Demonstrate that you understand the role of the Radius control plane, the `rad` CLI, workspaces, and environments, and how these pieces are used together when a developer runs `rad deploy`.

## Learning Resources

- [Radius documentation — Guides](https://docs.radapp.io/guides/) — entry point for installation and configuration guidance.
- [What is Radius?](https://docs.radapp.io/concepts/) — overview of Radius concepts, including the control plane, environments, and recipes.
- [Install the rad CLI](https://docs.radapp.io/installation/) — how to obtain and verify the Radius command-line tool on Windows, macOS, and Linux.
- [Install Radius on a Kubernetes cluster](https://docs.radapp.io/guides/operations/kubernetes/install/) — supported cluster types, required permissions, and installation options.
- [Radius workspaces](https://docs.radapp.io/guides/operations/workspaces/overview/) — what a workspace is and how it connects the `rad` CLI to a control plane.
- [Radius environments overview](https://docs.radapp.io/guides/deploy-apps/environments/overview/) — how environments relate to the control plane and why they matter for later challenges.
- [Kubernetes: Install and Set Up kubectl](https://kubernetes.io/docs/tasks/tools/) — if you still need to configure cluster access on your workstation.

## Tips

- Any CNCF-conformant Kubernetes cluster will work. If you don't have one handy, a local cluster such as **kind** or **k3d** is perfectly fine for this hack and is fast to reset if something goes wrong.
- The account you use to install Radius needs cluster-admin level permissions on the Kubernetes cluster, because the control plane installs CRDs and cluster-scoped resources.
- If the installer seems to hang, check the pods in the Radius namespace — image pulls on a fresh cluster can take a few minutes before everything becomes `Ready`.
- Each team member should create their **own** workspace on their workstation, but all of them should point at the **same** control plane so that the team shares a single environment in the next challenges.
- You can always start over: uninstalling Radius from the cluster and re-running the installer is a supported workflow and is much faster than debugging a half-broken install.

## Advanced Challenges (Optional)

Finished early? Try one or more of the following:

- Install Radius into a **different Kubernetes distribution** (for example, swap AKS for kind, or vice versa) and confirm the rest of the hack still works against it.
- Explore the Radius **dashboard** that is installed with the control plane and use it to inspect your workspace and environment visually.
- Write a short runbook for your team explaining how to **upgrade** or **uninstall** Radius cleanly, including what happens to existing environments and applications.
