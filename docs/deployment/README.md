# Deploy Capability Portfolios

A capability portfolio delivers a consistent set of platform capabilities across cloud and edge environments.

The six supported portfolios are: `min`, `core`, `ent` (non-AI tiers) and
`min-ai`, `core-ai`, `ent-ai` (AI tiers). Each higher tier is a superset of
the one below.

## Deploy via Helm

All portfolios ship from a single unified Helm chart, `adaptive-apps`,
published as an OCI artifact. Portfolio selection is a profile file
applied with `-f`.

### Chart layout

- `charts/adaptive-apps/Chart.yaml` — the unified chart
- `charts/adaptive-apps/profiles/<portfolio>.yaml` — per-portfolio feature
  toggles (`min.yaml`, `core.yaml`, `ent.yaml`, `min-ai.yaml`, `core-ai.yaml`,
  `ent-ai.yaml`)

### Install from source

```bash
helm install adaptive-apps charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/min.yaml \
  --namespace adaptive-apps --create-namespace
```

Swap the profile (`-f`) to install a different portfolio.

To configure Keycloak with [Active Directory federation](../authentication/keycloak-active-directory.md),
you'll need to mount a trusted certificate (see step 9–10 [here](../authentication/adds-vm.md))
as well as a hostname alias (see [collateral/sample-values.yaml](../../collateral/sample-values.yaml) as an example):

```bash
helm install adaptive-apps charts/adaptive-apps \
  -f charts/adaptive-apps/profiles/min.yaml \
  --namespace adaptive-apps --create-namespace \
  -f <values file> \
  --set-file keycloak.customCert.crt=collateral/dc.crt \
  --set keycloak.customCert.fileName=dc.crt
```

### Install from GHCR

```bash
helm pull oci://ghcr.io/<owner>/adaptive-apps/charts/adaptive-apps --version <chart-version>
tar -xzf adaptive-apps-<chart-version>.tgz
helm install adaptive-apps ./adaptive-apps \
  -f ./adaptive-apps/profiles/min.yaml \
  --namespace adaptive-apps --create-namespace
```

### Release pipeline

The workflow `.github/workflows/release-helm-charts.yml` releases the
unified `adaptive-apps` chart to GHCR and creates GitHub Releases.

Release trigger conditions:

- Manual: `workflow_dispatch`
- Automatic: push to `main` with changes under `charts/adaptive-apps/**`

Published location format:

- `oci://ghcr.io/<owner>/adaptive-apps/charts/adaptive-apps:<version>`
