# What The Hack - AdaptiveApps - Coach Guide

## Introduction

Welcome to the coach's guide for the AdaptiveApps What The Hack. Here you will find links to specific guidance for coaches for each of the challenges.

This hack includes an optional [lecture presentation](Lectures.pptx) that features short presentations to introduce key topics associated with each challenge. It is recommended that the host present each short presentation before attendees kick off that challenge.

**NOTE:** If you are a Hackathon participant, this is the answer guide. Don't cheat yourself by looking at these during the hack! Go learn something. :)

## Coach's Guides

- Challenge 00: **[Prerequisites - Ready, Set, GO!](./Solution-00.md)**
  - Prepare your workstation to work with Azure.
- Challenge 01: **[Prepare the Platforms](./Solution-01.md)**
	 - Set up and validate one or more target environments (AKS, Arc-enabled Kubernetes, Azure Local, or local Kubernetes).
- Challenge 02: **[Deploy and Explore Radius](./Solution-02.md)**
	 - Install Radius, create environments, and explore the Radius application model and portal.
- Challenge 03: **[Build the Platform Abstractions](./Solution-03.md)**
	 - Create or import Radius resource types and recipes to define reusable platform capabilities.
- Challenge 04: **[Build the Platform Abstractions with Recipes](./Solution-04.md)**
	 - Build the Platform Abstractions with Recipes.
- Challenge 05: **[Port the App Across Environments](./Solution-05.md)**
	 - Deploy the same application to a second environment with minimal configuration changes.
- Challenge 06: **[Adapt Identity Services - Configure User Authentication](./Solution-06.md)**
	 - Configure authentication using different identity providers such as Entra ID, Keycloak, or Active Directory.
- Challenge 07: **[Secure Service Communication - Configure Service-to-Service Communication](./Solution-07.md)**
	 - Implement workload identities and mTLS to secure service-to-service communication.
- Challenge 08: **[Adapt AI Services](./Solution-08.md)**
	 - Switch between cloud-hosted and local AI models while keeping the application unchanged.
- Challenge 09: **[Extend the Application Model with Custom Radius Resources](./Solution-09.md)**
	 - Create a custom Radius resource type and recipe to add a new reusable platform capability.
- Challenge 10: **[Modernize a Brownfield Application](./Solution-10.md)**
	 - Convert an existing application into an Adaptive App and deploy it consistently across environments.
- Challenge 21 *(optional)*: **[Define a PostgreSQL Resource Type and Author Recipes for Azure and Azure Local](./Solution-21.md)**
	 - Optional Module — deep-dive extension for use during a Micro Hack.
- Challenge 22 *(optional)*: **[Deploy a Portable .NET 10 Web App on Azure and Azure Local with Radius](./Solution-22.md)**
	 - Optional Module — deep-dive extension for use during a Micro Hack.

The guide covers the common preparation steps a coach needs to do before any What The Hack event, including how to properly configure Microsoft Teams.

### Student Resources

Before the hack, it is the Coach's responsibility to download and package up the contents of the `/Student/Resources` folder of this hack into a "Resources.zip" file. The coach should then provide a copy of the Resources.zip file to all students at the start of the hack.

Always refer students to the [What The Hack website](https://aka.ms/wth) for the student guide: [https://aka.ms/wth](https://aka.ms/wth)

**NOTE:** Students should **not** be given a link to the What The Hack repo before or during a hack. The student guide does **NOT** have any links to the Coach's guide or the What The Hack repo on GitHub.

### Additional Coach Prerequisites (Optional)

_Please list any additional pre-event setup steps a coach would be required to set up such as, creating or hosting a shared dataset, or deploying a lab environment._

## Azure Requirements

This hack requires students to have access to an Azure subscription where they can create and consume Azure resources. These Azure requirements should be shared with a stakeholder in the organization that will be providing the Azure subscription(s) that will be used by the students.

_Please list Azure subscription requirements._

_For example:_

- Azure resources that will be consumed by a student implementing the hack's challenges
- Azure permissions required by a student to complete the hack's challenges.

## Suggested Hack Agenda (Optional)

_This section is optional. You may wish to provide an estimate of how long each challenge should take for an average squad of students to complete and/or a proposal of how many challenges a coach should structure each session for a multi-session hack event. For example:_

- Sample Day 1
  - Challenge 1 (1 hour)
  - Challenge 2 (30 mins)
  - Challenge 3 (2 hours)
- Sample Day 2
  - Challenge 4 (45 mins)
  - Challenge 5 (1 hour)
  - Challenge 6 (45 mins)

## Repository Contents

_The default files & folders are listed below. You may add to this if you want to specify what is in additional sub-folders you may add._

- `./Coach`
  - Coach's Guide and related files
- `./Coach/Solutions`
  - Solution files with completed example answers to a challenge
- `./Student`
  - Student's Challenge Guide
- `./Student/Resources`
  - Resource files, sample code, scripts, etc meant to be provided to students. (Must be packaged up by the coach and provided to students at start of event)
