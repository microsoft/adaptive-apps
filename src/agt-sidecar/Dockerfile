# syntax=docker/dockerfile:1.7
#
# agt-sidecar — Agent Governance Toolkit (AGT) "OpenClaw sidecar" image
# for the adaptive-apps trading sample.
#
# Why this Dockerfile exists
#   The upstream agent-governance-toolkit does NOT publish a public sidecar
#   image (see https://microsoft.github.io/agent-governance-toolkit/deployment/openclaw-sidecar/).
#   The documented "no Docker" install path is
#
#     pip install agent-os-kernel
#     python -m agent_os.server --host 0.0.0.0 --port 8081
#
#   This Dockerfile wraps exactly that recipe so the adaptive-apps build-and-
#   publish workflow can ship a deterministically tagged sidecar image to GHCR
#   alongside backend / ai-agent / frontend.
#
# The image is consumed by the Radius.Resources/agentGuardrails Recipe via the
# `defaultImage` parameter and injected as a sidecar container into the
# ai-agent pod by radius/app.bicep (runtimes.kubernetes.pod.containers).
#
# Runtime contract (must match what radius/app.bicep wires in):
#   • listens on PROXY_PORT (default 8081) for the governance HTTP API
#   • mounts the policies ConfigMap at /policies (read-only)
#   • reads LOG_LEVEL / POLICY_DIR / AGT_MODE env vars

ARG PYTHON_VERSION=3.12-slim

FROM python:${PYTHON_VERSION} AS base

# Pin the upstream package to a known-good version. Bump intentionally — the
# AGT sidecar API surface is still evolving (see upstream Roadmap section).
ARG AGENT_OS_KERNEL_VERSION=0.3.0

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    POLICY_DIR=/policies \
    LOG_LEVEL=INFO \
    HOST=0.0.0.0 \
    PORT=8081

# Create unprivileged user the sidecar will run as. The mounted policies
# ConfigMap is read-only so no write access to /policies is required.
RUN groupadd --system --gid 10001 agt \
 && useradd  --system --uid 10001 --gid agt --home-dir /home/agt --create-home agt \
 && mkdir -p /policies \
 && chown -R agt:agt /policies

RUN pip install "agent-os-kernel==${AGENT_OS_KERNEL_VERSION}"

USER agt
WORKDIR /home/agt

EXPOSE 8081 9091

# Health probes match what the upstream docs publish:
#   GET /health  → liveness
#   GET /ready   → readiness
# Kubernetes probes are wired in app.bicep on the injected sidecar
# container, so the HEALTHCHECK here is only for `docker run` smoke tests.
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD python -c "import urllib.request,os; urllib.request.urlopen(f'http://127.0.0.1:{os.environ.get(\"PORT\",\"8081\")}/health').read()" \
    || exit 1

ENTRYPOINT ["python", "-m", "agent_os.server"]
CMD ["--host", "0.0.0.0", "--port", "8081"]
