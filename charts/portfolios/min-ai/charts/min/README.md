# min Capability Portfolio Chart

This folder contains a single Helm chart for the `min` portfolio.

## Portfolio chart

- Chart path: `charts/portfolios/min`
- Chart name: `min`

## Included components

- Keycloak identity provider (`components.keycloak.enabled=true`)

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
