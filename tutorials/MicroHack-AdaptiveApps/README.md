# What The Hack - AdaptiveApps

## Introduction

Modern organizations increasingly need to run the *same* application in very different places — a public cloud region, an on-premises datacenter, an edge location, or a fully disconnected site — without rebuilding it for each one. This hack uses [Radius](https://radapp.io), the open-source, cloud-native application platform, to make a single application **adaptive**: it asks for the capabilities it needs (a database, a message broker, a workload identity, an AI model) while each environment decides *how* those capabilities are provided.

Across the challenges you take on the role of a **platform engineering** team. You prepare target platforms, install Radius, define reusable platform abstractions (resource types and recipes), and then prove portability by deploying a sample stock-trading application across multiple environments with no changes to the application model. Later challenges extend the same pattern to identity, secure service-to-service communication, AI services, and brownfield modernization.

## Learning Objectives

In this hack you will learn how a platform team uses Radius to deliver portable, policy-driven application platforms across cloud, on-premises, and edge environments. You will:

1. Prepare one or more Kubernetes target platforms and install the Radius control plane. As an optional workshop convenience, two AKS clusters can be used as separate logical environments when Azure Local or another edge cluster is not available.
2. Define platform abstractions — portable Radius resource types and the recipes that implement them.
3. Deploy a single application model across multiple environments with minimal, environment-only configuration changes.
4. Adapt cross-cutting concerns — identity providers, workload identity and secure service communication, and AI services — without changing the application.

## Challenges

- Challenge 00: **[Prerequisites - Ready, Set, GO!](Student/Challenge-00.md)**
	 - Prepare your workstation to work with Azure.
- Challenge 01: **[Prepare the Platforms](Student/Challenge-01.md)**
	 - Set up and validate one or more target environments (AKS, Arc-enabled Kubernetes, Azure Local, or local Kubernetes).
- Challenge 02: **[Deploy and Explore Radius](Student/Challenge-02.md)**
	 - Install Radius, create environments, and explore the Radius application model and portal.
- Challenge 03: **[Build the Platform Abstractions](Student/Challenge-03.md)**
	 - Create or import Radius resource types and recipes to define reusable platform capabilities.
- Challenge 04: **[Build the Platform Abstractions with Recipes](Student/Challenge-04.md)**
	 - Author and register Radius recipes that implement the resource type contracts as concrete infrastructure.
- Challenge 05: **[Port the App Across Environments](Student/Challenge-05.md)**
	 - Deploy the same application to a second environment with minimal configuration changes.
- Challenge 06: **[Adapt Identity Services - Configure User Authentication](Student/Challenge-06.md)**
	 - Configure authentication using different identity providers such as Entra ID, Keycloak, or Active Directory.
- Challenge 07: **[Secure Service Communication - Configure Service-to-Service Communication](Student/Challenge-07.md)**
	 - Implement workload identities and mTLS to secure service-to-service communication.
- Challenge 08: **[Adapt AI Services](Student/Challenge-08.md)**
	 - Switch between cloud-hosted and local AI models while keeping the application unchanged.
- Challenge 09: **[Extend the Application Model with Custom Radius Resources](Student/Challenge-09.md)**
	 - Create a custom Radius resource type and recipe to add a new reusable platform capability.
- Challenge 10: **[Modernize a Brownfield Application](Student/Challenge-10.md)**
	 - Convert an existing application into an Adaptive App and deploy it consistently across environments.
- Challenge 21 *(optional)*: **[Define a PostgreSQL Resource Type and Author Recipes for Azure and Azure Local](Student/Challenge-21.md)**
	 - Optional Module — deep-dive extension for use during a Micro Hack. Can be completed independently or as a follow-on to the core challenge track.
- Challenge 22 *(optional)*: **[Deploy a Portable .NET 10 Web App on Azure and Azure Local with Radius](Student/Challenge-22.md)**
	 - Optional Module — deep-dive extension for use during a Micro Hack. Can be completed independently or as a follow-on to the core challenge track.

## Prerequisites

- An Azure subscription with **Owner** access (to create resource groups and role assignments)
- A CNCF-conformant Kubernetes cluster you can target (Azure Kubernetes Service is recommended; kind/k3d, Arc-enabled Kubernetes, or Azure Local also work). For the portability challenge, a second target environment is optional but useful; a second AKS cluster can be used as a lab stand-in for local/edge when needed.
- [Azure CLI](https://learn.microsoft.com/cli/azure/install-azure-cli)
- [kubectl](https://kubernetes.io/docs/tasks/tools/)
- [Helm](https://helm.sh/docs/intro/install/)
- [Radius CLI (`rad`)](https://docs.radapp.io/getting-started/install/)
- [Visual Studio Code](https://code.visualstudio.com)
- A bash or PowerShell 7 shell (on Windows, WSL2 or PowerShell 7 both work)

See [Challenge 00](Student/Challenge-00.md) for detailed, per-platform installation and verification steps.

## Contributors

- The Adaptive Apps team (Microsoft)
- Dylan de Jong
- Jan Egil Ring
- Wesley Backelant

- Contributions welcome — see the [adaptive-apps repository](https://github.com/microsoft/adaptive-apps).
