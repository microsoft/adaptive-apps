# The Ent Portfolio

The Ent Portfolio is designed for enterprise workloads that require
fine-grained, policy-as-code authorization in addition to the mesh
capabilities provided by [the Core Portfolio](./core.md). It extends
Core with a centralized policy decision point integrated with the
service mesh.

## Components

* [Core Portfolio Components](./core.md#components)
* +[Open Policy Agent (OPA)](https://www.openpolicyagent.org/) with the [opa-envoy-plugin](https://github.com/open-policy-agent/opa-envoy-plugin), deployed as a centralized external authorization service and registered with Istio as a mesh `extensionProvider`.

## Additional Guidance

* [`adaptive-apps` chart](../../charts/adaptive-apps/) — install with `-f charts/adaptive-apps/profiles/ent.yaml`. See the [governance tutorial](../../tutorials/governance/README.md) for a sample `AuthorizationPolicy` that delegates decisions to OPA.
* [Getting started with the Ent portfolio](../../tutorials/getting-started/README.md) (set `PORTFOLIO=ent`)
* [Policies overview](../policies/README.md) — rationale for OPA as the default policy engine.
