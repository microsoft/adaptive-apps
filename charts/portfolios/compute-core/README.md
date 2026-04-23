# compute-core Capability Portfolio Chart

This folder contains a single Helm chart for the `compute-core` portfolio.

## Portfolio chart

- Chart path: `charts/portfolios/compute-core`
- Chart name: `compute-core`

## Included components

- Keycloak identity provider (`components.keycloak.enabled=true`)

Additional `compute-core` capabilities can be added as optional components under the same chart.

## Release target

The portfolio chart is published as an OCI artifact to:

- `ghcr.io/<owner>/portable-apps/charts/portfolios/compute-core`
