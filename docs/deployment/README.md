# Deploy Capability Portfolios

A capability porfolio delivers a consistent set of platform capabilities across cloud and edge environments. We currently define two portfolios: `compute-core` and `compute-ai`. `compute-core` defines capabilities that support a typical enterprise application, including compute, identiy, messaging, state and observability. `compute-ai` extends `compute-core` with additional AI-related capabilities like manging agents and AI models.

## Deploy via Helm

Capability portfolios can be packaged as Helm charts and published as OCI artifacts.

### Current chart layout

- `charts/portfolios/compute-core`: Portfolio chart for `compute-core`

Current components included in the portfolio chart:

- Keycloak identity provider

### Install compute-core portfolio chart from source

```bash
helm install compute-core charts/portfolios/compute-core \
	--namespace compute-core \
	--create-namespace
```

To configure Keycloak with [Active Directory federation](../authentication/keycloak-active-directory.md), you'll need to mount a trusted certificate (see step 9-10 [here](../authentication/adds-vm.md)) as well as hostname alias (see collateral/sample-values.yaml as an example):

```bash
helm install compute-core charts/portfolios/compute-core \
	--namespace compute-core \
	--create-namespace \
    -f <values file> \
    --set-file keycloak.customCert.crt=collateral/dc.crt \
    --set keycloak.customCert.fileName=dc.crt
```

### Install compute-core portfolio chart from GHCR

```bash
helm pull oci://ghcr.io/<owner>/portable-apps/charts/portfolios/compute-core --version <chart-version>
helm install compute-core ./compute-core-<chart-version>.tgz \
	--namespace compute-core \
	--create-namespace
```

### Release pipeline

The workflow `.github/workflows/release-helm-charts.yml` releases portfolio charts to GHCR and creates GitHub Releases.

Release trigger conditions:

- Manual: `workflow_dispatch`
- Automatic: push to `main` with changes under `charts/portfolios/**`

Published location format:

- `oci://ghcr.io/<owner>/portable-apps/charts/portfolios/<portfolio>:<version>`

