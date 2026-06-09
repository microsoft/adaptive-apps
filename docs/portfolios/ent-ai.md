# The Ent-AI Portfolio

The Ent-AI Portfolio combines the [Min-AI Portfolio](./min-ai.md) and the [Ent Portfolio](./ent.md) to enable enterprise-grade deployments of AI-powered solutions with policy-based authorization in the service mesh.

## Components

* [Ent Portfolio Components](./ent.md#components)
* [Min-AI Portfolio Components](./min-ai.md#components)
* **Agent guardrails (recommended default)** — the [Agent Governance Toolkit](https://microsoft.github.io/agent-governance-toolkit/) sidecar runs in the AI agent pod (built from [`src/agt-sidecar/`](../../src/agt-sidecar/) and provisioned by the [`Radius.Resources/agentGuardrails`](../../radius/recipes/agent-guardrails/) recipe). Provides prompt-injection scanning and governed tool execution at the application layer, complementing the mesh-layer OPA PDP from `ent`. Turn it on at app deploy time with `--parameters enableAgentGuardrails=true` — the portfolio's `features.agentGuardrails.enabled=true` marker indicates this is the recommended default.

## Additional Guidance

* [`adaptive-apps` chart](../../charts/adaptive-apps/) — install with `-f charts/adaptive-apps/profiles/ent-ai.yaml`.
* [Getting started with the Ent-AI portfolio](../../tutorials/getting-started/README.md) (set `PORTFOLIO=ent-ai`)