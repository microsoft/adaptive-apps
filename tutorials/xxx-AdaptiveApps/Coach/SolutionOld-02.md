# Challenge 02 - Deploy and Explore Radius - Coach's Guide

[< Previous Solution](./Solution-01.md) - **[Home](./README.md)** - [Next Solution >](./Solution-03.md)

## Notes & Guidance

- This challenge follows directly from Challenge 1 — teams must have a healthy Kubernetes cluster and all supporting Azure resources (ACR, Key Vault) in place before starting.
- The single most important outcome: every team member can run `rad` commands against a shared Radius control plane, and the team understands the relationship between the `rad` CLI, workspaces, environments, and the Radius control plane components running in the cluster.
- The Radius control plane installs CRDs and cluster-scoped resources, so the user running `rad install kubernetes` must have `cluster-admin` on the AKS cluster. Using the AKS cluster admin credentials (`az aks get-credentials --admin`) is the simplest way to unblock teams.
- Only one team member needs to run `rad install kubernetes` against the shared AKS cluster. Each team member *does* need to set up their own local workspace — that is per-workstation, not per-cluster.
- Typical blockers to watch for:
  - Mismatched `kubectl` context — teams sometimes install Radius into a leftover local cluster. Have them run `kubectl config current-context` before `rad install kubernetes`.
  - AKS node image pulls can take 5–10 minutes before all Radius pods are `Ready`. Tell teams to wait rather than re-running the installer.
  - `rad` CLI version mismatch with the control plane chart — always install the latest CLI **and** let `rad install kubernetes` pick the matching chart. Do not pin a chart version unless you have a reason to.
- The **Explore** part matters: teams should open the Radius dashboard, inspect the environment, and be able to describe what each control plane component does. Do not let teams skip this step and jump straight to recipes.
- Expected time to complete for a team: **30–45 minutes**. Coach should wait ~15 minutes of apparent inactivity before stepping in.

## Solution Guide

### Stage 1 — Install the Radius control plane

On each workstation, install the `rad` CLI and verify the version:

```bash
wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
rad version
```

With the AKS cluster as the current `kubectl` context, install the Radius control plane:

```bash
rad install kubernetes --set rp.publicEndpointOverride=localhost:8081
```

This deploys the Radius control plane (applications-rp, controller, bicep-de, UCP, dashboard, etc.) into the `radius-system` namespace. Verify all pods are healthy:

```bash
kubectl get pods -n radius-system
kubectl get crds | grep radapp.io
```

Only one team member needs to run `rad install kubernetes` against the shared AKS cluster; everyone else reuses that install.

### Stage 2 — Post-install configuration (workspace + environment) and exploration

#### Understanding the Radius hierarchy

Before creating anything, coaches should land the full three-level hierarchy with the team. The relationship is:

```
Radius control plane (one per cluster)
│
├── Environment: env-azure-prod       ← platform + stage
├── Environment: env-azure-nonprod
├── Environment: env-local-prod
└── Environment: env-local-nonprod
```

Within **each** environment, resource groups organise workloads by domain or team — and applications live inside those groups:

```
env-azure-prod
│
├── Resource Group: rg-finance
│   ├── Application: app-finance-api
│   └── Application: app-finance-web
│
├── Resource Group: rg-hr
│   ├── Application: app-hr-api
│   └── Application: app-hr-web
│
└── Resource Group: rg-sales
    ├── Application: app-crm-api
    └── Application: app-crm-web
```

This means resource groups are **domain/team scopes inside an environment**, not environment scopes themselves.

#### Understanding Workspaces

A Radius **workspace** is a local, per-workstation configuration entry that tells the `rad` CLI which Kubernetes cluster (and therefore which Radius control plane) to target. It lives in `~/.rad/config.yaml` — not in the cluster. One workspace = one cluster = one control plane. All environments on that cluster are reachable once the workspace is active; you switch between them with `rad env switch`.

Because each environment already encodes the platform *and* the stage in its name, the recommended pattern is **one workspace per cluster**, named to match:

| Workspace | Kubernetes cluster | Environments hosted |
|---|---|---|
| `ws-azure-prod` | AKS cluster — Azure prod | `env-azure-prod` |
| `ws-azure-nonprod` | AKS cluster — Azure nonprod | `env-azure-nonprod` |
| `ws-local-prod` | AKS cluster — Azure Local prod | `env-local-prod` |
| `ws-local-nonprod` | AKS cluster — Azure Local nonprod | `env-local-nonprod` |

> **Coaching tip:** If teams ask "why not one workspace per environment?", point out that each workspace requires its own Kubernetes cluster and Radius control plane install. Clusters are expensive; environments are a single `rad env create` command.

With this layout, switching targets from the command line is explicit and readable:

```bash
# Create and deploy to Azure prod
rad workspace create kubernetes ws-azure-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-azure-prod
rad env switch env-azure-prod
rad group switch rg-finance
rad deploy ./app.bicep

# Deploy the same app to Azure Local nonprod — zero changes to app.bicep
rad workspace create kubernetes ws-local-nonprod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-nonprod
rad env switch env-local-nonprod
rad group switch rg-finance
rad deploy ./app.bicep
```

#### Recommended naming convention

| Object | Pattern | Examples |
|---|---|---|
| Workspace | `ws-<platform>-<stage>` | `ws-azure-prod`, `ws-local-nonprod` |
| Environment | `env-<platform>-<stage>` | `env-azure-prod`, `env-local-nonprod` |
| Resource group | `rg-<domain>` | `rg-finance`, `rg-hr`, `rg-sales` |
| Application | `app-<domain>-<component>` | `app-finance-api`, `app-hr-web` |

Keeping the prefix consistent (`ws-`, `env-`, `rg-`, `app-`) makes `rad list` output immediately scannable and avoids confusion between Radius objects and Azure resource groups.

#### Understanding Resource Groups

A Radius **resource group** is a logical container inside the UCP that groups applications and other Radius resources by domain or team *within* an environment. It is **not** an Azure resource group and has no Azure billing or policy implications.

Key points to coach:

- Resource groups are **domain/team scopes**, not environment scopes. Create them once and reuse them across environments (`rg-finance` exists in `env-azure-prod` *and* in `env-local-nonprod`).
- Every Radius application must live in a resource group. You must create one (and switch to it) before deploying an app.
- Switching with `rad group switch` changes the scope for all subsequent `rad` commands on that workstation.

> **Coaching tip:** Radius resource groups and Azure resource groups serve analogous *organisational* purposes but at different scopes. Radius groups organise the Radius application model; Azure groups organise Azure infrastructure. A single Radius environment can target many different Azure resource groups via recipe parameters.

#### Understanding Environments

A Radius **environment** is a named configuration that tells Radius *how* to deploy applications on a specific target platform. It carries:

- The **Kubernetes namespace** where application workloads are placed.
- An optional **cloud provider registration** (Azure subscription + resource group) that recipes use when provisioning managed cloud resources.
- The **recipe registrations** that map portable resource types (e.g. `Radius.Resources/postgreSQL`) to their platform-specific implementations.

Environments are the bridge between the portable application model and the concrete infrastructure beneath it. The same `app.bicep` deployed to `env-azure-prod` and `env-local-nonprod` produces completely different infrastructure — the environment selects which recipes run.

The full picture, combining all naming layers:

| Workspace | Environment | Kubernetes namespace | Cloud provider |
|---|---|---|---|
| `ws-azure-prod` | `env-azure-prod` | `prod` | Azure subscription / prod RG |
| `ws-azure-nonprod` | `env-azure-nonprod` | `nonprod` | Azure subscription / nonprod RG |
| `ws-local-prod` | `env-local-prod` | `prod` | *(none — in-cluster recipes only)* |
| `ws-local-nonprod` | `env-local-nonprod` | `nonprod` | *(none — in-cluster recipes only)* |

> **Coaching tip:** The environment name and Kubernetes namespace do not have to match, but aligning them (as above) makes it immediately clear where a workload landed.

#### Action

The target structure to build for this challenge (Azure production and Azure Local production, three domain teams each):

```
ws-azure-prod  (AKS — Azure)
└── env-azure-prod
    ├── rg-finance
    │   ├── app-finance-api
    │   └── app-finance-web
    └── rg-hr
        ├── app-hr-api
        └── app-hr-web

ws-local-prod  (AKS — Azure Local)
└── env-local-prod
    ├── rg-finance
    │   ├── app-finance-api
    │   └── app-finance-web
    └── rg-hr
        ├── app-hr-api
        └── app-hr-web
```

**Step 1 — Azure production workspace, environment, and resource groups**

On each workstation, create the workspace pointing at the Azure AKS cluster:

```bash
rad workspace create kubernetes ws-azure-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-azure-prod
```
Create the domain resource groups:

```bash
rad group create rg-finance
rad group create rg-hr
```

Create the environment:

```bash
rad env create env-azure-prod --group rg-finance --namespace prod
rad env switch env-azure-prod
rad group switch rg-finance
```



Register the Azure cloud provider:

```bash
rad env update env-azure-prod \
    --azure-subscription-id "$AZURE_SUBSCRIPTION" \
    --azure-resource-group "$RESOURCE_GROUP"
```


**Step 2 — Azure Local production workspace, environment, and resource groups**

⚠️ **Critical:** If you are running both environments on the **same Kubernetes cluster** (same control plane), you must use **different Kubernetes namespaces** for each environment. If you are on a **different cluster**, proceed to Step 2a below.

**Step 2a — If on a different cluster:**

Switch kubectl context to the Azure Local AKS cluster, then verify you are not still on the Azure prod cluster context:

```bash
kubectl config get-contexts
kubectl config current-context
```

If this context is the same one used for `ws-azure-prod`, switch to the Azure Local / Arc context first. Otherwise `ws-local-prod` will point to the same Radius control plane and environment creation will fail with a namespace conflict.

Then create and switch the local workspace:

```bash
rad workspace create kubernetes ws-local-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-prod
```

Create the same domain resource groups, then create the environment. Use namespace `prod-local` to avoid conflicts if on the same control plane (or `prod` if on a different cluster):

```bash
rad group create rg-finance
rad group create rg-hr
rad group switch rg-finance
rad env create env-local-prod --group rg-finance --namespace prod-local
rad env switch env-local-prod
```

Do not register the Azure cloud provider on `env-local-prod` in this challenge. Keep it in-cluster only so dashboard validation remains consistent (`env-azure-prod` has Azure provider, `env-local-prod` does not).

Verify everything is wired up correctly:

```bash
rad workspace switch ws-azure-prod
rad env switch env-azure-prod
rad workspace list
rad env list
rad group list
```

You should see:
- `ws-azure-prod` workspace showing `env-azure-prod` as its active environment
- Both `env-azure-prod` and `env-local-prod` listed with status `Succeeded`
- `rg-finance` and `rg-hr` listed as resource groups

Have teams open the **Radius dashboard** and explore the environment, resource groups, and empty application list:

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open `http://localhost:7007` in a browser.

> **AKS only — register the Azure credential.** The `wi-helper.sh` you ran
> from [`tutorials/getting-started/assets/wi-helper.sh`](../../getting-started/assets/wi-helper.sh) during
> [`prepare-aks.md`](../../common/prepare-aks.md) created an Entra app named
> `${AKS_CLUSTER}-radius-app` and federated it to the Radius service
> accounts. Now bind it to the Radius control plane (`ada bootstrap
> --with-radius --platform aks` does this for you):
>
> ```bash
> export APPLICATION_CLIENT_ID=$(az ad app list \
>   --query "[?displayName=='${AKS_CLUSTER}-radius-app'].appId | [0]" -o tsv)
> export TENANT_ID=$(az account show --query tenantId -o tsv)
>
> rad credential register azure wi \
>   --client-id "$APPLICATION_CLIENT_ID" --tenant-id "$TENANT_ID"
>
> # Verify (may take 30+ seconds to refresh):
> rad credential show azure
> ```


Ask them to identify which components were installed and what each one does. Stop here — recipe authoring, environment customization, and app deployment are the next challenges.


### Stage 3 — Exploring the Radius Dashboard

The Radius dashboard is a built-in web UI that ships with every Radius control plane install. It gives a visual overview of environments, resource groups, applications, and deployed resources — without needing to run `rad` CLI commands. Teams should explore it before moving to recipe authoring so they can see the structure they just created.

#### Connect via port-forward

The dashboard runs as a pod in the `radius-system` namespace and is not exposed externally by default. Connect from your workstation with:

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open **http://localhost:7007** in your browser.

#### What to explore

Once connected, have teams work through each section:

**Environments**

- Navigate to **Environments** and verify that `env-azure-prod` and `env-local-prod` are listed.
- Click into each environment and confirm:
  - The correct Kubernetes namespace (`prod`) is shown.
  - The Azure cloud provider is registered on `env-azure-prod` and absent on `env-local-prod`.
  - The recipe list is empty — recipes will be added in Challenge 3.

**Resource groups**

- Navigate to **Resource Groups** and confirm `rg-finance` and `rg-hr` appear under each environment.
- Note that there are no applications yet — that is expected at this stage.

**Applications**

- The **Applications** section should be empty. Point out that this is where deployed apps will appear in later challenges, each linked back to the environment and resource group it was deployed into.

**Connections and recipes**

- Both sections will be empty. Use this as a coaching moment to preview what teams will populate in Challenge 3 (recipes) and Challenge 4 (applications with connections).

#### Coaching questions to ask during exploration

- *"What is the difference between an environment and a resource group in what you can see here?"*
- *"Why does `env-local-prod` have no cloud provider registered?"*
- *"Where would you look in the dashboard to confirm a deployment succeeded?"*
- *"If a recipe fails, what information do you think would appear here?"*

#### Switching workspace context in the dashboard

The dashboard is scoped to the control plane of the **current workspace**. To explore the Azure Local structure, switch workspace and relaunch:

```bash
rad workspace switch ws-local-prod
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Verify that `env-local-prod`, `rg-finance`, and `rg-hr` are now visible and that `env-azure-prod` is no longer listed — it lives on a different control plane.
## Sample deployment script

The script below performs stage 1 (Radius control plane install) and stage 2 (workspaces, environments, resource groups) for both the Azure production and Azure Local production clusters. It assumes Challenge 1 is complete. Hand it to teams only if they are stuck.

```bash
# ---------------------------------------------------------------------------
# Stage 1 — Install the Radius control plane
# Run this once, on the cluster that should host the control plane.
# ---------------------------------------------------------------------------

# Install the rad CLI (Linux / macOS / WSL). On Windows use the PowerShell
# install script from https://docs.radapp.io/installation/ instead.
if ! command -v rad >/dev/null 2>&1; then
    echo "Installing rad CLI..."
    wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
fi
rad version

echo "Installing Radius control plane into AKS..."
rad install kubernetes

echo "Waiting for Radius pods to become Ready..."
kubectl wait --for=condition=Ready pods --all -n radius-system --timeout=10m
kubectl get pods -n radius-system

# ---------------------------------------------------------------------------
# Stage 2a — Azure production: workspace, environment, resource groups
# Assumes the current kubectl context points at the Azure prod AKS cluster.
# ---------------------------------------------------------------------------

# Variables — update to match your Challenge 1 values
azure_rg=radiushack-rg
sub_id=$(az account show --query id -o tsv)

echo "--- Azure production ---"
rad workspace create kubernetes ws-azure-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-azure-prod

rad group create rg-finance
rad group create rg-hr
rad group switch rg-finance

rad env create env-azure-prod --group rg-finance --namespace prod
rad env switch env-azure-prod

rad env update env-azure-prod \
    --azure-subscription-id "$sub_id" \
    --azure-resource-group "$azure_rg"

echo "Verifying Azure prod..."
rad workspace list
rad env list
rad group list

# ---------------------------------------------------------------------------
# Stage 2b — Azure Local production: workspace, environment, resource groups
# Assumes kubectl now points at the Azure Local / Arc-enabled Kubernetes cluster.
# If you intentionally reuse the same AKS control plane, use a distinct namespace
# such as prod-local to avoid colliding with env-azure-prod.
# ---------------------------------------------------------------------------

echo "--- Azure Local production ---"
kubectl config current-context

rad workspace create kubernetes ws-local-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-prod

rad group create rg-finance
rad group create rg-hr
rad group switch rg-finance

rad env create env-local-prod --group rg-finance --namespace prod-local
rad env switch env-local-prod

echo "Verifying Azure Local prod..."
rad workspace list
rad env list
rad group list

# ---------------------------------------------------------------------------
# AKS only — register the Azure credential with the Radius control plane.
# The helper from tutorials/getting-started/assets/wi-helper.sh creates an
# Entra app named ${AKS_CLUSTER}-radius-app and federates the Radius service
# accounts. Bind that app to Radius after the control plane is installed.
# ---------------------------------------------------------------------------

export APPLICATION_CLIENT_ID=$(az ad app list \
  --query "[?displayName=='${AKS_CLUSTER}-radius-app'].appId | [0]" -o tsv)
export TENANT_ID=$(az account show --query tenantId -o tsv)

rad workspace switch ws-azure-prod
rad credential register azure wi \
  --client-id "$APPLICATION_CLIENT_ID" --tenant-id "$TENANT_ID"

# Verify (may take 30+ seconds to refresh):
rad credential show azure
```
