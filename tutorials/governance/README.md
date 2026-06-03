# Governance (OPA + Istio) — Operator Tutorial

This tutorial covers authoring request-level allow/deny rules on top of the
Istio service mesh. It picks up where the
[getting-started](../getting-started/README.md) tutorial leaves off and
complements the [service mesh tutorial](../service-mesh/README.md) (which
covers mTLS, traffic shaping, and troubleshooting).

> **Applies to:**
>
> | Portfolio | This tutorial covers |
> | --- | --- |
> | `core`, `core-ai` | Native Istio `AuthorizationPolicy` only (no OPA installed) |
> | `ent`, `ent-ai` | Native Istio `AuthorizationPolicy` **and** OPA-delegated `action: CUSTOM` policies |
> | `min`, `min-ai` | Not applicable (no Istio) |

Prerequisites:

* You completed the [getting-started](../getting-started/README.md) tutorial.
* The sample app is running at `trading-adaptive-apps` with sidecars (`2/2`
  pods).
* `kubectl` is pointed at the cluster.

```bash
export APP_NAMESPACE=trading-adaptive-apps
export NAMESPACE=adaptive-apps               # where the portfolio chart was installed
```

## Contents

1. [Two ways to author allow/deny](#1-two-ways-to-author-allowdeny)
2. [Native `AuthorizationPolicy` (all Istio portfolios)](#2-native-authorizationpolicy-all-istio-portfolios)
3. [OPA-delegated `action: CUSTOM` (`ent` / `ent-ai`)](#3-opa-delegated-action-custom-ent--ent-ai)
4. [Writing and updating Rego policies](#4-writing-and-updating-rego-policies)
5. [Observing decisions](#5-observing-decisions)
6. [Troubleshooting](#6-troubleshooting)

---

## 1. Two ways to author allow/deny

Istio's `AuthorizationPolicy` resource supports two enforcement models:

| Model | `action:` | Where the decision is made | When to use |
| --- | --- | --- | --- |
| **Native** | `ALLOW` / `DENY` | Inside the Envoy sidecar, using the rules declared in the policy | Simple matches on source identity, headers, methods, paths |
| **OPA-delegated** | `CUSTOM` + `provider.name: opa-ext-authz-grpc` | Sidecar forwards the request attributes to OPA via gRPC; OPA returns allow/deny based on Rego | Complex rules, shared policy logic, decision logging, dynamic data |

`core` / `core-ai` portfolios only have the native form available.
`ent` / `ent-ai` install OPA and register it as an Istio `extensionProvider`,
so both forms work — the OPA-delegated form is the **intended** pattern for
non-trivial rules on those portfolios.

---

## 2. Native `AuthorizationPolicy` (all Istio portfolios)

`AuthorizationPolicy` is "allow all" by default; once you write a policy, the
default flips to "deny" for the matched workloads.

### 2.1 Block a specific header

This example denies any request to `frontend` carrying `x-deny: true`:

```bash
cat <<EOF | kubectl apply -f -
apiVersion: security.istio.io/v1
kind: AuthorizationPolicy
metadata:
  name: frontend-deny-bad-header
  namespace: $APP_NAMESPACE
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: frontend
  action: DENY
  rules:
  - when:
    - key: request.headers[x-deny]
      values: ["true"]
EOF
```

Test from inside the mesh (so the sidecar enforces the policy):

```bash
kubectl -n $APP_NAMESPACE run curlbox \
  --image=curlimages/curl:8.10.1 --restart=Never \
  --command -- sleep infinity

# Allowed
kubectl -n $APP_NAMESPACE exec curlbox -c curlbox -- \
  curl -sI http://frontend:3000/ | head -1
# HTTP/1.1 200 OK   (or 302 if redirected to OIDC)

# Denied
kubectl -n $APP_NAMESPACE exec curlbox -c curlbox -- \
  curl -sI -H 'x-deny: true' http://frontend:3000/ | head -1
# HTTP/1.1 403 Forbidden

kubectl -n $APP_NAMESPACE delete pod curlbox
kubectl -n $APP_NAMESPACE delete authorizationpolicy frontend-deny-bad-header
```

### 2.2 Restrict who can call `backend`

Only the `frontend` and `ai-agent` workloads should reach the backend:

```bash
cat <<EOF | kubectl apply -f -
apiVersion: security.istio.io/v1
kind: AuthorizationPolicy
metadata:
  name: backend-allow-known-callers
  namespace: $APP_NAMESPACE
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: backend
  action: ALLOW
  rules:
  - from:
    - source:
        principals:
        - cluster.local/ns/$APP_NAMESPACE/sa/frontend
        - cluster.local/ns/$APP_NAMESPACE/sa/ai-agent
EOF
```

Any sidecar-enabled pod with a different service account that tries to call
`backend` will get `403 RBAC: access denied`.

---

## 3. OPA-delegated `action: CUSTOM` (`ent` / `ent-ai`)

> **Skip this section** if you're on `core` / `core-ai` — OPA is not installed
> there.

### 3.1 Confirm OPA is wired up

The `ent` chart's `post-install-istio-extauthz` Job patches Istio's
`MeshConfig` to register OPA as an `extensionProvider`. Verify it:

```bash
kubectl -n istio-system get configmap istio -o jsonpath='{.data.mesh}' \
  | grep -A3 extensionProviders
# Expect an entry named `opa-ext-authz-grpc` pointing at
# opa.<release-namespace>.svc.cluster.local:9191
```

If the entry is missing, the post-install Job didn't run — confirm the gates
are on:

```bash
helm get values $RELEASE -n $NAMESPACE | grep -A2 governance
# Expect:
#   governance:
#     opa: true
#     istioExtAuthz: true
```

### 3.2 Apply a `CUSTOM` policy that delegates to OPA

```bash
cat <<EOF | kubectl apply -f -
apiVersion: security.istio.io/v1
kind: AuthorizationPolicy
metadata:
  name: frontend-ext-authz
  namespace: $APP_NAMESPACE
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: frontend
  action: CUSTOM
  provider:
    name: opa-ext-authz-grpc
  rules:
  - to:
    - operation:
        methods: ["*"]
EOF
```

With `action: CUSTOM`, the sidecar forwards **every** request matching the
rules to OPA over gRPC. OPA's response decides allow vs deny; the sidecar
returns `403` for denies.

---

## 4. Writing and updating Rego policies

OPA loads its policy bundle from the `opa-policies` ConfigMap (mounted into
the OPA pod). Replace its contents to roll out a new policy.

### 4.1 Author a policy

```bash
cat > opa-policy.rego <<'EOF'
package envoy.authz

import rego.v1

default allow := true

# Deny any request that carries the x-deny header.
allow := false if {
  input.attributes.request.http.headers["x-deny"] == "true"
}
EOF
```

### 4.2 Load it into OPA

```bash
kubectl -n $NAMESPACE create configmap opa-policies \
  --from-file=policy.rego=opa-policy.rego \
  --dry-run=client -o yaml | kubectl apply -f -

# OPA hot-reloads policy ConfigMaps; no restart needed.
```

### 4.3 Test the policy

```bash
kubectl -n $APP_NAMESPACE run curlbox \
  --image=curlimages/curl:8.10.1 --restart=Never \
  --command -- sleep infinity

# Allowed
kubectl -n $APP_NAMESPACE exec curlbox -c curlbox -- \
  curl -sI http://frontend:3000/ | head -1

# Denied
kubectl -n $APP_NAMESPACE exec curlbox -c curlbox -- \
  curl -sI -H 'x-deny: true' http://frontend:3000/ | head -1

kubectl -n $APP_NAMESPACE delete pod curlbox
```

---

## 5. Observing decisions

```bash
# OPA writes a decision log line per evaluation
kubectl -n $NAMESPACE logs -f deployment/opa | grep decision_id

# Native AuthorizationPolicy denials surface in the sidecar access log
kubectl -n $APP_NAMESPACE logs -f deploy/frontend -c istio-proxy \
  | grep -E 'RBAC|denied|403'

# Snapshot of policies that apply to a workload (requires istioctl)
istioctl x authz check deploy/frontend.$APP_NAMESPACE
```

---

## 6. Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `CUSTOM` policy applied but every request is allowed | `extensionProviders` entry missing in MeshConfig | Re-run the chart with `--set features.governance.opa=true --set features.governance.istioExtAuthz=true`; confirm the post-install Job succeeded |
| OPA logs show "no policy loaded" | `opa-policies` ConfigMap missing or empty | Re-apply the ConfigMap; check `kubectl -n $NAMESPACE get cm opa-policies -o yaml` |
| Rego change didn't take effect | OPA didn't pick up the ConfigMap update (volume cache) | `kubectl -n $NAMESPACE rollout restart deployment/opa` |
| Native `ALLOW` policy locks out everything | Selector matched a wider set of pods than expected; remember that adding an `ALLOW` policy implicitly denies everything not matched | Narrow the `selector.matchLabels`, or add a complementary `ALLOW` for system traffic |
| 403 on internal calls *after* enabling OPA | OPA gRPC service unreachable from sidecars | `kubectl -n $NAMESPACE get svc opa`; check port `9191`; check `istioctl proxy-config cluster deploy/frontend.$APP_NAMESPACE \| grep opa` |

---

For mTLS, traffic shaping, and general mesh troubleshooting, see the
[service mesh tutorial](../service-mesh/README.md).
