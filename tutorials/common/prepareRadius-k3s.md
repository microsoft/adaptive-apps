# Prepare Radius on k3s (Local)

## Prerequisites

- Healthy k3s cluster from [prepare-k3s.md](./prepare-k3s.md) with `kubectl` context active.
- `rad` CLI not yet installed (or version will be auto-updated).
- `cluster-admin` access on the k3s cluster.

## Stage 1 — Install Radius control plane

### 1.1 Install the rad CLI

```bash
wget -q "https://raw.githubusercontent.com/radius-project/radius/main/deploy/install.sh" -O - | /bin/bash
rad version
```

### 1.2 Install Radius into the k3s cluster

```bash
rad install kubernetes
```

This deploys the Radius control plane (applications-rp, controller, bicep-de, UCP, dashboard, etc.) into the `radius-system` namespace.

### 1.3 Verify all Radius pods are healthy

```bash
kubectl get pods -n radius-system
kubectl get crds | grep radapp.io
```

Expected: All pods in `radius-system` should be `Running` or `Completed`. CRDs should include `applications.radapp.io`, `environments.radapp.io`, etc.

## Stage 2 — Configure workspaces and environments

### 2.1 Create workspace for k3s local environment

```bash
rad workspace create kubernetes ws-local-prod \
    --context "$(kubectl config current-context)" --force
rad workspace switch ws-local-prod
```

### 2.2 Create resource groups

```bash
rad group create rg-finance
rad group create rg-hr
rad group switch rg-finance
```

### 2.3 Create environment

```bash
rad env create env-local-prod --group rg-finance --namespace prod
rad env switch env-local-prod
```

Do not register any cloud provider for local environments — all recipes run in-cluster only.

## Stage 3 — Validate setup

```bash
rad workspace list
rad env list
rad group list
```

Expected output:
- `ws-local-prod` workspace active
- `env-local-prod` environment listed with status `Succeeded`
- `rg-finance` and `rg-hr` resource groups listed

## Stage 4 — Optional: Explore the dashboard

```bash
kubectl port-forward svc/dashboard -n radius-system 7007:80
```

Then open **http://localhost:7007** in a browser and verify:
- Environments tab shows `env-local-prod`
- Resource groups tab shows `rg-finance` and `rg-hr`
- Applications tab is empty (expected at this stage)

## Notes

- k3s is lightweight and ideal for development/testing. For production-grade testing, use AKS or Arc-enabled clusters.
- All workloads deploy into the `prod` Kubernetes namespace by default. Change by modifying `--namespace prod` above.
- If you need multiple environments on the same k3s cluster (e.g., prod and nonprod), use different namespaces:
  ```bash
  rad env create env-local-nonprod --group rg-finance --namespace nonprod
  ```

## Next steps

Proceed to Challenge 2 — Recipe authoring and application deployment.
