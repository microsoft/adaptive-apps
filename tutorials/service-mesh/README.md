# Service Mesh — Operator Tutorial

This tutorial picks up where the [getting-started](../getting-started/README.md)
tutorial leaves off. It assumes you've already:

* Installed an Istio-enabled portfolio (`core`, `ent`, `core-ai`, or `ent-ai`).
* Deployed the sample app to the `trading-adaptive-apps` namespace.
* Confirmed the frontend works at <http://localhost:3000>.

If any of the above isn't true, finish the [getting-started](../getting-started/README.md) tutorial first.

## 0. Prerequisites

* `kubectl` pointed at the cluster where the portfolio is installed.
* (Recommended) `istioctl` matching the chart's Istio version
  (`v1.27.0` by default). Install per [Istio docs](https://istio.io/latest/docs/setup/getting-started/#download).

```bash
export NAMESPACE=adaptive-apps               # where the portfolio chart was installed
export APP_NAMESPACE=trading-adaptive-apps   # where the sample app runs
export ISTIO_NAMESPACE=istio-system          # for AKS: aks-istio-system
```


## 1. What the chart already did for you

With `features.istio.install=true` and `features.istio.mtls=true` (both on
for `core`/`ent` profiles), the chart's hooks ran two jobs:

| Hook | Result |
| --- | --- |
| `pre-install-istio` Job | Installs `istio-base` and `istiod` v1.27.0 into `$ISTIO_NAMESPACE` via `helm install` |
| `post-install-peer-auth` Job | Applies a mesh-wide `PeerAuthentication` named `default` in `$ISTIO_NAMESPACE` with `mtls.mode: STRICT` |

The sidecar injection is the **only** part the chart can't do for you, because
Radius owns the app namespace. That's the next step.

> **AKS:** When you set `features.istio.install=false` + `istio.namespace=aks-istio-system`,
> the chart skips the pre-install Job and lets the AKS managed Istio add-on own
> `istiod`. The post-install `PeerAuthentication` Job still runs, but it
> targets `aks-istio-system`.

## 2. Enroll the app namespace in the mesh

Radius creates the app namespace (`trading-adaptive-apps`) without the
`istio-injection=enabled` label, and the chart can't label it for you because
Radius owns the namespace lifecycle. Label it manually and restart the
workloads so they come back with sidecars:

```bash
kubectl label namespace $APP_NAMESPACE istio-injection=enabled --overwrite
kubectl rollout restart deployment -n $APP_NAMESPACE
```

Wait for every pod to reach `2/2` ready (the second container is the
`istio-proxy` sidecar):

```bash
kubectl get pods -n $APP_NAMESPACE -w
```

> **Why both steps?** The label only affects **new** pods — Istio injects the
> sidecar at admission time. The rollout restart recreates the existing pods
> so they pick up sidecars.

> **AKS:** The managed Istio add-on uses revision labels instead of
> `istio-injection=enabled`. Use `istio.io/rev=asm-1-27` (or whichever
> revision your add-on installed) and the same rollout restart.

## 3. Verify the mesh

```bash
# 1. istiod is running
kubectl get pods -n $ISTIO_NAMESPACE -l app=istiod

# 2. Every app pod has 2/2 containers (app + istio-proxy sidecar)
kubectl get pods -n $APP_NAMESPACE
# READY column should be 2/2 for frontend, backend, ai-agent, postgres, etc.

# 3. (with istioctl) Confirm proxies are synced with istiod
istioctl proxy-status
# All entries should show SYNCED for CDS, LDS, EDS, RDS.
```

If `proxy-status` shows `STALE` or `NOT SENT` for a pod, see
[Troubleshooting](#6-troubleshooting).


## 4. Verify mTLS is enforced

The mesh-wide `PeerAuthentication` requires mTLS for every pod-to-pod call
inside the mesh. Two ways to confirm:

```bash
# A. Inspect the policy
kubectl get peerauthentication -n $ISTIO_NAMESPACE default -o yaml
# Look for: spec.mtls.mode: STRICT
```

```bash
# B. Ask Envoy how it's talking to a destination. `istioctl x describe`
#    summarizes the effective auth + routing config for a workload,
#    including whether traffic to each port is mTLS-wrapped.
FRONTEND_POD=$(kubectl -n $APP_NAMESPACE get pod \
  -o jsonpath='{.items[?(@.metadata.labels.radapp\.io/resource=="frontend")].metadata.name}' \
  | awk '{print $1}')
# Fallback if your labels differ — just grep by name:
[ -z "$FRONTEND_POD" ] && FRONTEND_POD=$(kubectl -n $APP_NAMESPACE get pod \
  -o name | grep -m1 frontend | cut -d/ -f2)
istioctl x describe pod -n $APP_NAMESPACE $FRONTEND_POD
# Look for a line like:
#   Pilot reports that pod enforces mTLS and clients speak mTLS
```

**Negative test** — temporarily create a pod *outside* the mesh and confirm it
can't reach backend services:

```bash
# Launch a plain pod (no sidecar) in a non-mesh namespace
kubectl create namespace mesh-test 2>/dev/null || true
kubectl -n mesh-test run probe \
  --image=curlimages/curl:8.10.1 --restart=Never \
  --command -- sleep infinity

# This should fail with a connection reset or RBAC denial
kubectl -n mesh-test exec probe -- \
  curl -sS --max-time 5 http://backend.$APP_NAMESPACE.svc.cluster.local:5000/ || \
  echo "(expected) blocked by STRICT mTLS"

# Cleanup
kubectl delete namespace mesh-test
```

## 5. Shape traffic with `VirtualService` and `DestinationRule`

The chart doesn't ship any `VirtualService` / `DestinationRule` — every Service
in the mesh gets an implicit one. You can author your own to add timeouts,
retries, fault injection, or traffic splits.

### 5.1 Add a 2-second timeout to `backend` calls

```bash
cat <<EOF | kubectl apply -f -
apiVersion: networking.istio.io/v1
kind: VirtualService
metadata:
  name: backend
  namespace: $APP_NAMESPACE
spec:
  hosts: [backend]
  http:
  - route:
    - destination:
        host: backend
        port:
          number: 5000
    timeout: 2s
    retries:
      attempts: 3
      perTryTimeout: 1s
      retryOn: 5xx,reset,connect-failure
EOF
```

### 5.2 Inject a 5% fault to verify the retry budget

```bash
kubectl -n $APP_NAMESPACE patch virtualservice backend --type=merge -p '
spec:
  http:
  - fault:
      abort:
        percentage: { value: 5 }
        httpStatus: 503
    route:
    - destination:
        host: backend
        port: { number: 5000 }
    retries:
      attempts: 3
      perTryTimeout: 1s
      retryOn: 5xx,reset,connect-failure
'

# Generate load and confirm the failure rate stays low thanks to retries.
# `curlbox` must be IN the mesh (otherwise STRICT mTLS resets the call),
# so request sidecar injection explicitly via annotation and wait for 2/2.
kubectl -n $APP_NAMESPACE run curlbox \
  --image=curlimages/curl:8.10.1 --restart=Never \
  --annotations='sidecar.istio.io/inject=true' \
  --command -- sleep infinity
kubectl -n $APP_NAMESPACE wait --for=condition=Ready pod/curlbox --timeout=60s
# Confirm the sidecar is actually attached — should print "2/2":
kubectl -n $APP_NAMESPACE get pod curlbox -o \
  jsonpath='{.status.containerStatuses[*].ready}{"\n"}'

for i in $(seq 1 100); do
  kubectl -n $APP_NAMESPACE exec curlbox -c curlbox -- \
    curl -sS -o /dev/null -w '%{http_code}\n' http://backend:8080/healthz
done | sort | uniq -c
# Expect mostly 200, with the occasional 503 squelched by the 3-retry policy.

kubectl -n $APP_NAMESPACE delete pod curlbox
kubectl -n $APP_NAMESPACE delete virtualservice backend
```

### 5.3 Pin client side load balancing with `DestinationRule`

```bash
cat <<EOF | kubectl apply -f -
apiVersion: networking.istio.io/v1
kind: DestinationRule
metadata:
  name: backend
  namespace: $APP_NAMESPACE
spec:
  host: backend
  trafficPolicy:
    loadBalancer:
      simple: LEAST_REQUEST
    connectionPool:
      tcp: { maxConnections: 100 }
      http:
        http2MaxRequests: 1000
        maxRequestsPerConnection: 50
    outlierDetection:
      consecutive5xxErrors: 5
      interval: 30s
      baseEjectionTime: 60s
EOF
```

---

## 6. Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| Pods show `1/1` instead of `2/2` in `READY` | Namespace label `istio-injection=enabled` missing, or pods predate the label | `kubectl label namespace $APP_NAMESPACE istio-injection=enabled --overwrite && kubectl rollout restart deployment -n $APP_NAMESPACE` |
| `istioctl proxy-status` shows `STALE` for a workload | The sidecar can't reach `istiod` (network policy, wrong port) | Check istiod logs: `kubectl -n $ISTIO_NAMESPACE logs deploy/istiod` |
| 503 UC / connection reset between mesh pods | mTLS mismatch — one side isn't in the mesh | Verify both pods are `2/2`; check `PeerAuthentication` is `STRICT` only where you expect it |
| `AuthorizationPolicy` not taking effect | Pod created before the policy, or selector doesn't match | `kubectl rollout restart deployment <name> -n $APP_NAMESPACE`; double-check `selector.matchLabels` |
| Post-install peer-auth Job stuck | `peerauthentications.security.istio.io` CRD missing — istiod not ready | `kubectl get crd peerauthentications.security.istio.io`; if missing, re-run `helm upgrade` |
| AKS: `PeerAuthentication` job applied to wrong namespace | Forgot `--set istio.namespace=aks-istio-system` | `helm upgrade $RELEASE charts/adaptive-apps --reuse-values --set istio.namespace=aks-istio-system` |

**Useful one-liners:**

```bash
# Dump the effective Envoy config for a pod
istioctl proxy-config all deploy/frontend.$APP_NAMESPACE

# Show which policies apply to a pod
istioctl x authz check deploy/frontend.$APP_NAMESPACE

# Watch live access logs from a sidecar
kubectl -n $APP_NAMESPACE logs -f deploy/frontend -c istio-proxy

# Summarize a pod's effective auth + routing (includes mTLS status)
istioctl x describe pod -n $APP_NAMESPACE <pod-name>
```

---

## 7. What's next

* **Governance (OPA)** — author `AuthorizationPolicy` resources, either
  natively (`core` / `core-ai`) or delegated to OPA via `action: CUSTOM`
  (`ent` / `ent-ai`). See the [governance tutorial](../governance/README.md)
  *(coming soon)*.
* **Observability** — query distributed traces in Zipkin and Envoy metrics in
  Prometheus to see policies and timeouts in action. *(tutorial coming)*
* **Istio docs** — for advanced topics (egress gateways, multi-cluster mesh,
  request authentication with JWTs), see <https://istio.io/latest/docs/>.
