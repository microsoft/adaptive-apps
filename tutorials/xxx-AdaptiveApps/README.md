# What The Hack - AdaptiveApps

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
- Challenge 01: **[Install and Configure the Radius Control Plane](Student/Challenge-01.md)**
	 - Prepare a Kubernetes cluster, install the `rad` CLI, and deploy the Radius control plane and an initial environment that will be used by the later challenges.
- Challenge 02: **[Define a PostgreSQL Resource Type and Author Recipes for Azure and Azure Local](Student/Challenge-02.md)**
	 - This challenge is where the *platform engineering* story of Radius really clicks for attendees: they move from consuming a pre-built environment (Challenge 01) to **defining a new abstraction** (a `postgreSQL` resource type) and then **wiring up two very different implementations of that abstraction** — one that calls Azure to provision a managed Azure Database for PostgreSQL flexible server, and one that runs PostgreSQL as a container on AKS for an "Azure Local" / on-premises-style environment. The application developer in later challenges will consume *one* resource type and never know (or care) which implementation was chosen
- Challenge 03: **[Deploy a Portable .NET 10 Web App on Azure and Azure Local with Radius](Student/Challenge-03.md)**
	 - In Challenge 02 the team built the *data tier* of the platform: one resource type, two recipes, identical schema, two completely different runtimes. This challenge is the **application tier counterpart**. Teams now define a `Radius.Resources/webApp` resource type for a .NET 10 web application, author one recipe that lands the app on **Azure App Service** (Linux, container-based) using an Azure Verified Module, and a second recipe that lands the same container on AKS as a `Deployment` + `Service` + `Ingress` for the **Azure Local** environment.
- Challenge 04: **[Title of Challenge](Student/Challenge-04.md)**
	 - Description of challenge
- Challenge 05: **[Title of Challenge](Student/Challenge-05.md)**
	 - Description of challenge
- Challenge 06: **[Title of Challenge](Student/Challenge-06.md)**
	 - Description of challenge
- Challenge 07: **[Title of Challenge](Student/Challenge-07.md)**
	 - Description of challenge
- Challenge 08: **[Title of Challenge](Student/Challenge-08.md)**
	 - Description of challenge
- Challenge 09: **[Title of Challenge](Student/Challenge-09.md)**
	 - Description of challenge
- Challenge 10: **[Title of Challenge](Student/Challenge-10.md)**
	 - Description of challenge

## Prerequisites

- Your own Azure subscription with Owner access
- Visual Studio Code
- Azure CLI
- An AVNET X231 device

## Contributors

- Jane Q. Public
- Joe T. Muppet
