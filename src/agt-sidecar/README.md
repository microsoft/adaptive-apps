# agt-sidecar

Container image for the **Agent Governance Toolkit (AGT)** in-pod sidecar used
by the adaptive-apps trading sample. The sidecar provides:

- Prompt-injection detection (`POST /api/v1/detect/injection`)
- Governed action execution (`POST /api/v1/execute`)
- Health/readiness probes (`GET /health`, `GET /ready`)
- JSON metrics (`GET /api/v1/metrics`)

## Why this image exists

The upstream
[agent-governance-toolkit](https://microsoft.github.io/agent-governance-toolkit/deployment/openclaw-sidecar/)
does **not** publish a public sidecar image yet. Its documented "without
Docker" install path is `pip install agent-os-kernel`. This Dockerfile wraps
exactly that recipe so the project's `build-and-publish.yml` workflow can
ship a deterministically tagged image to GHCR.

## How it's consumed

Two pieces of the repo work together to deliver guardrails:

1. **Radius Recipe** [`kubernetes-agt-sidecar.bicep`](../../radius/recipes/agent-guardrails/kubernetes-agt-sidecar.bicep)
   stages a `policies` ConfigMap and outputs the recommended image, ports, and
   ConfigMap name.
2. **Application Bicep** [`app.bicep`](../../radius/app.bicep) injects this
   image as a sidecar container on the `ai-agent` pod via
   `runtimes.kubernetes.pod`, mounting the ConfigMap at `/policies`.

The agent and sidecar communicate over `localhost` — never expose this image
as a Kubernetes `Service` or a Radius `Gateway`. Doing so breaks the AGT
trust model (no mTLS, no auth between agent and sidecar).

## Local smoke test

```bash
docker build -t agt-sidecar:dev src/agt-sidecar/
docker run --rm -p 8081:8081 agt-sidecar:dev
curl -s http://localhost:8081/health
```
