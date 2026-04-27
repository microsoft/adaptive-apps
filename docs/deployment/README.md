# Deploy Capability Portfolios

A capability portfolio delivers a consistent set of platform capabilities across cloud and edge environments.

Current portfolio hierarchy:

- Non-AI: `min` -> `core` -> `ent`
- AI: `min-ai` -> `core-ai` -> `ent-ai`

`min` currently contains the capabilities previously packaged under `compute-core`.

## Deploy via Helm

Capability portfolios can be packaged as Helm charts and published as OCI artifacts.

### Current chart layout

- `charts/portfolios/min`: Base non-AI portfolio chart
- `charts/portfolios/core`: Builds on `min`
- `charts/portfolios/ent`: Builds on `core`
- `charts/portfolios/min-ai`: Base AI portfolio chart, builds on `min`
- `charts/portfolios/core-ai`: Builds on `min-ai`
- `charts/portfolios/ent-ai`: Builds on `core-ai`

Current components included in the portfolio chart:

- Keycloak identity provider

### Install min portfolio chart from source

```bash
helm install min charts/portfolios/min \
	--namespace min \
	--create-namespace
```

To configure Keycloak with [Active Directory federation](../authentication/keycloak-active-directory.md), you'll need to mount a trusted certificate (see step 9-10 [here](../authentication/adds-vm.md)) as well as hostname alias (see collateral/sample-values.yaml as an example):

```bash
helm install min charts/portfolios/min \
	--namespace min \
	--create-namespace \
    -f <values file> \
    --set-file keycloak.customCert.crt=collateral/dc.crt \
    --set keycloak.customCert.fileName=dc.crt
```

### Install min portfolio chart from GHCR

```bash
helm pull oci://ghcr.io/<owner>/portable-apps/charts/portfolios/min --version <chart-version>
helm install min ./min-<chart-version>.tgz \
	--namespace min \
	--create-namespace
```

### Release pipeline

The workflow `.github/workflows/release-helm-charts.yml` releases portfolio charts to GHCR and creates GitHub Releases.

Release trigger conditions:

- Manual: `workflow_dispatch`
- Automatic: push to `main` with changes under `charts/portfolios/**`

Published location format:

- `oci://ghcr.io/<owner>/portable-apps/charts/portfolios/<portfolio>:<version>`

