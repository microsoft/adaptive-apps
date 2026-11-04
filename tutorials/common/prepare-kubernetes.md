# Prepare Kubernetes

Use this guide to prepare a Kubernetes platform for Adaptive Apps. Start with
the common cluster and repository preparation, then complete only the
capability sections required by your application:

* GPU and KAITO for local model hosting.
* Locally trusted HTTPS for browser-facing development endpoints.
* Additional storage, network-policy, and admission requirements for
  Kubernetes-hosted portable agents.

## 1. Install workstation tools

Install:

* [Docker](https://docs.docker.com/)
* [Helm](https://helm.sh/docs/intro/install/)
* [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
* [Radius CLI](https://docs.radapp.io/guides/tooling/rad-cli/howto-rad-cli/)

Install `ada` separately by following [Prepare the CLI](prepare-cli.md).

## 2. Prepare the cluster

Choose the target platform and leave `kubectl` pointing at it:

* [Local k3s](prepare-k3s.md)
* [AKS](prepare-aks.md)
* [Azure Arc-enabled Kubernetes](prepare-arc.md)
* [Azure Local](prepare-azure-local.md)

If `ada bootstrap --platform k3s` will create local k3s, no separate cluster
creation is necessary.

Verify the selected context before deploying:

```bash
kubectl config current-context
kubectl version
kubectl get nodes
```

## 3. Prepare the repository chart

From the repository root, build the checked-in Helm dependencies:

```bash
helm dependency build charts/adaptive-apps --skip-refresh
```

Repeat this command only after using a fresh clone or changing chart
dependencies.

## 4. Optional: prepare local AI with GPU and KAITO

KAITO is an application model-hosting option, not a portable-agent
prerequisite. Any supported application or portfolio can use an external
model instead.

To host a model locally in Kubernetes, complete
[GPU and KAITO preparation](prepare-gpu-kaito.md). Selecting an `*-ai`
portfolio marks the AI capability but does not install KAITO automatically.

## 5. Optional: prepare locally trusted HTTPS

Locally trusted HTTPS is useful for any browser-facing development workload.
Trusting the mkcert CA is a one-time workstation setup. Creating certificate
files is a one-time step per clone because private keys are not committed.

Install [mkcert](https://github.com/FiloSottile/mkcert). Firefox on Linux also
requires `libnss3-tools`. Then, from the repository root:

```bash
install -d -m 700 .adaptive/private
mkcert -install
mkcert \
  -cert-file .adaptive/private/ui-tls.crt \
  -key-file .adaptive/private/ui-tls.key \
  localhost 127.0.0.1 ::1
chmod 600 .adaptive/private/ui-tls.crt .adaptive/private/ui-tls.key
```

These are ordinary TLS certificate and key files. A custom Kubernetes ingress
can consume them through a TLS Secret, for example:

```bash
kubectl create secret tls adaptive-local-tls \
  --cert=.adaptive/private/ui-tls.crt \
  --key=.adaptive/private/ui-tls.key \
  --namespace <application-namespace>
```

The integrated portable-agent sample uses the same certificate through its
Envoy Gateway:

* Trading UI: `https://localhost:3443`
* Agent UI: `https://localhost:8443`
* Keycloak: `https://localhost:8444`

For that sample, create the files before `ada init`. `ada init` recognizes
their names and retains them in the generated private inputs; `ada bootstrap`
later applies the `adaptive-ui-tls` Kubernetes Secret. The trading application
creates a cross-namespace `HTTPRoute` to its frontend Service, so no Caddy or
application-level TLS server is needed.

For an existing `agents.json`, make sure its browser origin matches the
Gateway listener:

```bash
python3 - <<'PY'
import json
from pathlib import Path

path = Path("agents.json")
config = json.loads(path.read_text())
config["tradingOrigin"] = "https://localhost:3443"
path.write_text(json.dumps(config, indent=2) + "\n")
PY
```

New configurations use that HTTPS origin by default. `ada init` propagates it
to both the frontend's `APP_BASE_URL` and the retained Keycloak
`adaptive-trading` client.

### WSL with a Windows browser

`mkcert -install` inside WSL trusts the CA only inside Linux. Either run mkcert
on Windows, or import the WSL CA shown by `mkcert -CAROOT` into Windows
**Trusted Root Certification Authorities**. In an elevated PowerShell:

```powershell
certutil -addstore -f Root "\\wsl.localhost\<distro>\home\<user>\.local\share\mkcert\rootCA.pem"
```

Replace `<distro>` and `<user>`, then restart the browser. Never distribute
`rootCA-key.pem`; it is the private key for your local development CA.

## 6. Optional: enable Kubernetes-hosted portable agents

This section contains the requirements specific to Kubernetes-hosted portable
agents:

* Terraform 1.10–1.16 for the Radius Terraform recipes.
* Kubernetes 1.33 or later with `ValidatingAdmissionPolicy`.
* A default, dynamically provisioning `StorageClass`. Each agent requests a
  `ReadWriteOnce` volume so state survives Pod replacement. The volume is not
  a backup and does not survive deletion of the volume, cluster, or underlying
  storage.
* A CNI that actively enforces Kubernetes `NetworkPolicy`. The API being
  present does not prove that allow and deny rules affect traffic.

Run the API and configuration checks against the intended context:

```bash
terraform version
kubectl version

kubectl api-resources \
  --api-group=admissionregistration.k8s.io |
  grep validatingadmissionpolicies

kubectl get storageclass \
  -o custom-columns='NAME:.metadata.name,DEFAULT:.metadata.annotations.storageclass\.kubernetes\.io/is-default-class,PROVISIONER:.provisioner'

kubectl api-resources \
  --api-group=networking.k8s.io |
  grep networkpolicies
```

The storage output must contain exactly one suitable default class with a real
provisioner, not `kubernetes.io/no-provisioner`.

### Verify dynamic storage

```bash
cat <<'EOF' | kubectl apply -f -
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: portable-agent-storage-check
  namespace: default
spec:
  accessModes: [ReadWriteOnce]
  resources:
    requests:
      storage: 1Mi
EOF

kubectl wait \
  --for=jsonpath='{.status.phase}'=Bound \
  pvc/portable-agent-storage-check \
  --namespace default \
  --timeout 2m
kubectl delete pvc portable-agent-storage-check --namespace default
```

### Verify NetworkPolicy enforcement

The first request establishes that networking works. The second must fail
after ingress is denied:

```bash
kubectl create namespace portable-agent-policy-check
trap 'kubectl delete namespace portable-agent-policy-check --ignore-not-found' EXIT
kubectl run policy-server \
  --namespace portable-agent-policy-check \
  --image=nginx:1.27-alpine \
  --labels=app=policy-server \
  --port=80
kubectl expose pod policy-server \
  --namespace portable-agent-policy-check \
  --port=80
kubectl run policy-client \
  --namespace portable-agent-policy-check \
  --image=curlimages/curl:8.12.1 \
  --restart=Never \
  --command -- sleep 3600
kubectl wait \
  --namespace portable-agent-policy-check \
  --for=condition=Ready pod/policy-server pod/policy-client \
  --timeout=2m

kubectl exec --namespace portable-agent-policy-check policy-client -- \
  curl --fail --silent --show-error --max-time 5 http://policy-server

cat <<'EOF' | kubectl apply -f -
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: deny-policy-server-ingress
  namespace: portable-agent-policy-check
spec:
  podSelector:
    matchLabels:
      app: policy-server
  policyTypes: [Ingress]
EOF

kubectl exec --namespace portable-agent-policy-check policy-client -- sh -c \
  'if curl --fail --silent --max-time 5 http://policy-server >/dev/null; then
     echo "ERROR: NetworkPolicy is not enforced"; exit 1
   else
     echo "NetworkPolicy is enforced"
   fi'

kubectl delete namespace portable-agent-policy-check
trap - EXIT
```

How these requirements are supplied depends on the platform:

| Platform | Portable-agent preparation |
| --- | --- |
| Local k3s through `ada`/k3d | Standard k3s includes the `local-path` provisioner and its NetworkPolicy controller. Do not use `--disable-network-policy`. |
| AKS | Use Azure CNI powered by Cilium. Managed CSI storage classes include a default class. |
| Existing Kubernetes, Arc-enabled Kubernetes, or Azure Local | Install a CSI provisioner with a default `StorageClass` and a policy-enforcing CNI such as Cilium or Calico, following the cluster vendor's production guidance. Connecting a cluster to Azure Arc does not add either capability. |

During `ada bootstrap --with agents`, `ada` installs the pinned Envoy Gateway
and agentgateway releases. It refuses to deploy over an installation owned by
another release.

