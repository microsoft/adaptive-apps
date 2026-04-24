# Setting up Microsoft Entra Connect

This document contains instructions of setting up a minimum test lab to sync from a Microsoft Entra tenant to a local Active Directory using Microsoft Entra Connect.


## Creatting a new Entra Tenant

1. Get a new Microsoft Account from https://signup.live.com
2. Get a free tenant from https://developer.microsoft.com/microsoft-365/dev-program

## Setting up Active Directory Domain Services (ADDS) on a VM

See instructions [here](./adds-vm.md).

## Setting up directory sync

1. Visit https://learn.microsoft.com/en-us/entra/identity/hybrid/install and follow instructions to install Microsoft Entra Connect Sync.
2. Follow the configuration wizard to configure directory sync.