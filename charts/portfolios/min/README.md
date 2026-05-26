# min Capability Portfolio Chart

This folder contains a single Helm chart for the `min` portfolio.

## Portfolio chart

- Chart path: `charts/portfolios/min`
- Chart name: `min`

## Included components

- Keycloak identity provider (`components.keycloak.enabled=true`, default)

## OIDC identity provider

The chart always renders a `<release>-oidc` ConfigMap that exposes the
resolved OIDC endpoints for downstream consumers (the `rad deploy app.bicep`
step in the tutorials, or other charts that depend on `min`). Keys:

| Key | Description |
|-----|-------------|
| `mode` | `keycloak` or `external` |
| `issuer` | OIDC issuer URL |
| `authEndpoint` | Authorization endpoint (in-cluster URL for Keycloak) |
| `browserAuthEndpoint` | Optional browser-facing override (port-forward, ingress) |
| `tokenEndpoint` | Token endpoint |
| `userInfoEndpoint` | Userinfo endpoint |
| `clientId` | OIDC client ID (if known at install time) |
| `clientSecretName` / `clientSecretKey` | Reference to the operator-managed Secret holding the client secret |

### Mode A — in-cluster Keycloak (default)

Endpoints are derived from the Keycloak Service. Override the realm via
`oidc.realm` (default `master`). Set `oidc.browserAuthEndpoint` when users
reach Keycloak at a different URL than in-cluster pods (e.g. via
`kubectl port-forward` or an ingress host).

### Mode B — external OIDC provider (Entra ID, Auth0, Okta, …)

Disable Keycloak and supply external endpoints:

```yaml
components:
  keycloak:
    enabled: false

oidc:
  clientId: <app-id-from-IdP>
  clientSecretRef:
    name: oidc-client     # Secret you create out-of-band
    key: clientSecret
  external:
    issuer: https://login.microsoftonline.com/<tenant>/v2.0
    authEndpoint: https://login.microsoftonline.com/<tenant>/oauth2/v2.0/authorize
    tokenEndpoint: https://login.microsoftonline.com/<tenant>/oauth2/v2.0/token
    userInfoEndpoint: https://graph.microsoft.com/oidc/userinfo
```

The chart does **not** create the client-secret Secret; render it yourself
(`kubectl create secret generic oidc-client --from-literal=clientSecret=...`)
and reference it via `oidc.clientSecretRef`.

### Optional Keycloak certificate mount

You can provide an optional certificate file (`.crt`) that will be stored in a
Kubernetes Secret and mounted into the Keycloak pod.

Example values override:

```yaml
keycloak:
	customCert:
		crt: |
			-----BEGIN CERTIFICATE-----
			MIIC...
			-----END CERTIFICATE-----
		fileName: custom-ca.crt
		mountPath: /opt/keycloak/conf/certs
```

When `keycloak.customCert.crt` is empty (default), no Secret or mount is created.

### Optional Keycloak hostname mapping (domain controller host)

You can inject static host-to-IP mappings into the Keycloak pod via Kubernetes
`hostAliases` so a domain controller hostname resolves without external DNS changes.

Example values override:

```yaml
keycloak:
	hostAliases:
		- ip: "10.0.0.10"
			hostnames:
				- dc01.contoso.local
```

This adds an `/etc/hosts` entry inside the Keycloak pod.

Additional `min` capabilities can be added as optional components under the same chart.

## Release target

The portfolio chart is published as an OCI artifact to:

- `ghcr.io/<owner>/adaptive-apps/charts/portfolios/min`
