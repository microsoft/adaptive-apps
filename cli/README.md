# `ada` — Adaptive Apps CLI

`ada` is the orchestration entry point for the Adaptive Apps platform. It
wraps `helm`, `kubectl`, and `rad` so installing a portfolio onto a target
context is a single command instead of a tutorial walkthrough.

## Status

Early scaffolding. Currently supports a single command:

```bash
ada bootstrap --portfolio <min|core|ent> [--platform <localk8s|aks|arc>] [--with ai]
```

## Build

```bash
cd cli
cargo build --release
./target/release/ada --help
```

## `ada bootstrap`

Installs or upgrades a portfolio onto the kube context currently selected by
`kubectl`. A thin `helm upgrade --install` wrapper.

By default the chart is pulled from the published OCI registry:

```
oci://ghcr.io/microsoft/adaptive-apps/charts/portfolios/<portfolio>[-ai]
```

at the version selected by `--version` (default `0.1.0`). Pass `--chart-root
<dir>` to install from a local chart source tree instead; the resolver
auto-detects the unified chart (`<chart-root>/adaptive-apps`) or falls back
to the legacy per-portfolio layout (`<chart-root>/portfolios/<portfolio>[-ai]`).

`--platform` is optional. When omitted no platform overrides are applied and
the install targets whatever cluster the current `kubectl` context points at.
When `--platform aks` is set, ada appends `--set istio.install.enabled=false`
and `--set istio.namespace=aks-istio-system` so the chart defers to the AKS
Istio add-on; pass `--azure-subscription <id>` to switch subscriptions via
`az account set` before invoking helm.

### Examples

Dry-run the published `ent + ai` chart against the current cluster:

```bash
ada bootstrap --portfolio ent --with ai --dry-run
```

Pin a specific chart version:

```bash
ada bootstrap --portfolio core --version 0.2.0
```

Install from a local checkout:

```bash
ada bootstrap --portfolio ent \
  --chart-root /path/to/adaptive-apps/charts
```

Install `core` onto AKS in a non-default namespace:

```bash
ada bootstrap --portfolio core --platform aks \
  --namespace platform --release platform
```

Pass through extra helm overrides:

```bash
ada bootstrap --portfolio min \
  -f my-overrides.yaml \
  --set global.imageRegistry=ghcr.io/myorg
```

### Prerequisites

`helm` and `kubectl` must be on `PATH`. The current `kubectl` context is the
target — switch contexts with `kubectl config use-context <name>` before
invoking `ada`.

## Roadmap

- `ada bootstrap` ✅ (this PR)
- `ada doctor` — prereq + cluster sanity checks
- `ada diff --from <p1> --to <p2>` — values + rendered manifest diff
- `ada explain --portfolio <p>` — human description of what's enabled
- `ada upgrade` / `ada teardown`

Tracked alongside the chart-unification work; see the parent repo discussion.
