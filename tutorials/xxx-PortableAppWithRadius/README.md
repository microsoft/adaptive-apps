# What The Hack - PortableAppWithRadius

## Introduction

The IoT Hack of the Century will take you on a whirlwind tour in the world of IoT and how it is being used in the modern world of mineral extraction in exotic locations like the Arctic and the wilds of South Africa.

## Learning Objectives

In this hack you will be solving the common business problem that companies in the mineral extraction industry face and how IoT solutions from Azure are brought to bare

1. Provision an IoT Hub
2. Set up an IoT Edge device
3. Bring Azure Sphere to your solution for scale and resiliency

## Challenges

- Challenge 00: **[Prerequisites - Ready, Set, GO!](Student/Challenge-00.md)**
	 - Prepare your workstation to work with Azure.
- Challenge 01: **[Install and Configure the Radius Control Plane Locally](Student/Challenge-01.md)**
	 - Prepare a local Kubernetes cluster, install the `rad` CLI, and deploy the Radius control plane and an initial environment that will be used by the later challenges.
- Challenge 02: **[Get Introduced to Radius Through the Quickstart](Student/Challenge-02.md)**
	 - Complete the [Quickstart](https://docs.radapp.io/quick-start/) to get a hands-on introduction to Radius.
- Challenge 03: **[Learn Radius Concepts by Building and Deploying an Application with the Tutorial](Student/Challenge-03.md)**
	 - This challenge is all about getting familiar with the core Radius concepts of Resource Types and Recipes and how they are used in Radius Applications and Environments. Study the Radius [Concepts](https://docs.radapp.io/concepts/) documentation and complete the end-to-end [tutorial](https://docs.radapp.io/tutorials/). The tutorial deploys an application that runs PostgreSQL as a container on a local Kubernetes cluster for an "Azure Local" or on-premises-style environment.
- Challenge 04: **[Install Radius on an Azure Kubernetes Service Cluster](Student/Challenge-04.md)**
	 - Install the Radius control plane on Azure Kubernetes Service (AKS) and set up [Azure cloud provider credentials](https://docs.radapp.io/guides/operations/providers/azure-provider/) within Radius to enable Azure deployments in later challenges.
- Challenge 05: **[Deploy the Quickstart Application on Azure Kubernetes Service with Radius](Student/Challenge-05.md)**
	 - Deploy the same application from Challenge 02, but this time target the AKS cluster to validate that the previously configured Azure credentials are working correctly.
- Challenge 06: **[Define a PostgreSQL Resource Type and Author Recipes for Azure and Azure Local](Student/Challenge-06.md)**
	 - This challenge is where the *platform engineering* story of Radius really clicks for attendees: they move from consuming a pre-built environment (Challenge 02) and deploying to local environments (Challenge 03) to creating an Azure cloud environment and wiring up a different implementation of the `postgreSQL` resource type abstraction that calls Azure to provision a managed Azure Database for PostgreSQL flexible server. The Azure Postgres recipes authored here will be used to redeploy the tutorial application from Challenge 03 to Azure instead of running locally, so the same application binds to a managed Azure Database for PostgreSQL flexible server through the `postgreSQL` resource type. The application developer in later challenges will consume one resource type and never know (or care) which implementation was chosen.
- Challenge 07: **[Explore the `adaptive-apps` Reference Application and Deploy It to Azure and Local Kubernetes](Student/Challenge-07.md)**
	 - In this challenge, attendees get hands-on with the [`adaptive-apps`](https://github.com/microsoft/adaptive-apps) reference application — a multi-component sample stock trading app (Node.js frontend, C# backend, PostgreSQL database, message bus, and OIDC identity provider) designed to deploy portably across Azure, Azure Arc, and local Kubernetes with platform-specific component mappings (e.g., Azure SQL vs. self-managed PostgreSQL, Azure Event Grid vs. Mosquitto, Microsoft Entra ID vs. Keycloak). Attendees study the reference application's architecture and deployment topology, then complete the [`min-getting-started`](https://github.com/microsoft/adaptive-apps/blob/microhack-EU/tutorials/min-getting-started/README.md) tutorial to deploy the app to both Azure and a local Kubernetes cluster. This sets the stage for the next few challenges, which use `adaptive-apps` as the basis for platform engineering and application development exercises with Radius.
<!-- - Challenge XX: **[Deploy a Portable .NET 10 Web App on Azure and Azure Local with Radius](Student/Challenge-XX.md)**
	 - In Challenge 02 the team built the *data tier* of the platform: one resource type, two recipes, identical schema, two completely different runtimes. This challenge is the **application tier counterpart**. Teams now define a `Radius.Resources/webApp` resource type for a .NET 10 web application, author one recipe that lands the app on **Azure App Service** (Linux, container-based) using an Azure Verified Module, and a second recipe that lands the same container on AKS as a `Deployment` + `Service` + `Ingress` for the **Azure Local** environment. -->
- Challenge 08: **[Title of Challenge](Student/Challenge-08.md)**
	 - Description of challenge
- Challenge 09: **[Title of Challenge](Student/Challenge-09.md)**
	 - Description of challenge
- Challenge 10: **[Title of Challenge](Student/Challenge-10.md)**
	 - Description of challenge
	 - Description of challenge
- Challenge 11: **[Title of Challenge](Student/Challenge-11.md)**
	 - Description of challenge

## Prerequisites

- Your own Azure subscription with Owner access
- Visual Studio Code
- Azure CLI
- An AVNET X231 device

## Contributors

- Jane Q. Public
- Joe T. Muppet
