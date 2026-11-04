# Advanced manual platform setup

This document preserves the manual setup path for custom automation,
troubleshooting, and learning how the standard trading platform is assembled.
The supported integrated portable-agent path uses `ada`; follow the
[getting-started tutorial](README.md) unless you intentionally need to own
these steps.

Before continuing, complete the applicable sections in
[Prepare Kubernetes](../common/prepare-kubernetes.md).

## 1. Install the portfolio chart

Set the same shell variables used by the main tutorial:

```bash
export PORTFOLIO=min
export RELEASE=$PORTFOLIO
export NAMESPACE=$PORTFOLIO
```

Install the selected profile:

```bash
helm install "$RELEASE" charts/adaptive-apps \
  -f "charts/adaptive-apps/profiles/$PORTFOLIO.yaml" \
  -n "$NAMESPACE" --create-namespace
```

On AKS, use the managed Istio add-on rather than installing Istio from the
chart:

```bash
helm install "$RELEASE" charts/adaptive-apps \
  -f "charts/adaptive-apps/profiles/$PORTFOLIO.yaml" \
  --set features.istio.install=false \
  --set istio.namespace=aks-istio-system \
  -n "$NAMESPACE" --create-namespace
```

Verify:

```bash
kubectl get pods -n "$NAMESPACE"
```

## 2. Create the standard trading OIDC client

Forward Keycloak:

```bash
kubectl port-forward -n "$NAMESPACE" "svc/$RELEASE-keycloak" 8080:8080
```

Open <http://localhost:8080>, sign in with the configured Keycloak
administrator, and create this client in the `master` realm:

1. **Clients → Create client**
   * Client type: **OpenID Connect**
   * Client ID: `adaptive-apps`
2. **Capability config**
   * Client authentication: **On**
   * Standard flow: enabled
3. **Login settings**
   * Valid redirect URIs: `http://localhost:3000/*`
4. Save and copy the secret from **Credentials**.

Export:

```bash
export OIDC_CLIENT_ID=adaptive-apps
export OIDC_CLIENT_SECRET=<paste-from-credentials-tab>
export OIDC_ISSUER="http://$RELEASE-keycloak.$NAMESPACE.svc.cluster.local:8080/realms/master"
export OIDC_AUTH_ENDPOINT="$OIDC_ISSUER/protocol/openid-connect/auth"
export OIDC_TOKEN_ENDPOINT="$OIDC_ISSUER/protocol/openid-connect/token"
export OIDC_USERINFO_ENDPOINT="$OIDC_ISSUER/protocol/openid-connect/userinfo"
export OIDC_BROWSER_AUTH_ENDPOINT=http://localhost:8080/realms/master/protocol/openid-connect/auth
```

## 3. Install and configure Radius

```bash
rad install kubernetes
rad workspace create kubernetes trading \
  --context "$(kubectl config current-context)" \
  --force
rad workspace switch trading
rad group create adaptive
rad group switch adaptive
rad env create trading --group adaptive --namespace trading
rad env switch trading
```

On AKS, register the workload-identity credential created by
[Prepare AKS](../common/prepare-aks.md):

```bash
export APPLICATION_CLIENT_ID=$(az ad app list \
  --display-name "$AKS_CLUSTER-radius-app" \
  --query '[].appId' -o tsv)
export TENANT_ID=$(az account show --query tenantId -o tsv)

rad credential register azure wi \
  --client-id "$APPLICATION_CLIENT_ID" \
  --tenant-id "$TENANT_ID"
rad credential show azure
```

Register the resource types and environment:

```bash
rad resource-type create -f radius/resource-types/types.yaml

# Local Kubernetes:
rad deploy radius/local-env.bicep --group adaptive

# AKS instead uses:
# rad deploy radius/aks-env.bicep --group adaptive
```

## 4. Deploy the standard trading sample

Choose a model binding:

```bash
# Local KAITO model:
AI_PARAMS=(--parameters aiProvider=local --parameters aiModel=Qwen/Qwen3-0.6B)

# Or OpenAI:
# AI_PARAMS=(--parameters aiProvider=openai --parameters aiModelName=gpt-4o --parameters aiApiKey=<key>)
```

Deploy:

```bash
rad deploy radius/app.bicep \
  --group adaptive \
  --environment trading \
  --parameters imageRegistry=ghcr.io/microsoft/adaptive-apps \
  --parameters imageTag=latest \
  --parameters authUsername=admin \
  --parameters authPassword=admin \
  --parameters "otelCollectorEndpoint=http://otel-collector.$NAMESPACE:4318" \
  --parameters "oidcIssuer=$OIDC_ISSUER" \
  --parameters oidcIssuerOverride=http://localhost:8080/realms/master \
  --parameters "oidcAuthEndpoint=$OIDC_AUTH_ENDPOINT" \
  --parameters "oidcBrowserAuthEndpoint=$OIDC_BROWSER_AUTH_ENDPOINT" \
  --parameters "oidcTokenEndpoint=$OIDC_TOKEN_ENDPOINT" \
  --parameters "oidcUserInfoEndpoint=$OIDC_USERINFO_ENDPOINT" \
  --parameters "oidcClientId=$OIDC_CLIENT_ID" \
  --parameters "oidcClientSecret=$OIDC_CLIENT_SECRET" \
  "${AI_PARAMS[@]}"
```

Expose the frontend and browse to <http://localhost:3000>:

```bash
rad resource expose Applications.Core/containers frontend \
  -a adaptive-apps \
  --port 3000 \
  --remote-port 3000
```

For a custom portable-agent installation, use
[Portable agents on Radius](../../docs/agents/native-radius.md). That path
requires explicit ownership of retained credentials, immutable recipes,
Gateway components, and agent identities; it is intentionally not duplicated
here.
