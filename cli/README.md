# `ada` — Adaptive Apps CLI

`ada` is the orchestration entry point for the Adaptive Apps platform. It
wraps `helm`, `kubectl`, and `rad` so installing a portfolio onto a target
context is a single command instead of a tutorial walkthrough.

## Status

Early scaffolding. Currently supports a single command:

```bash
ada bootstrap --portfolio <min|core|ent> [--platform <localk8s|aks|arc>] [--with ai]
```

## Install

One-line installers download the latest signed release archive and lay
the binary + bundled Radius artifacts out under `~/.adaptive` (the
default `$ADA_HOME`):

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/microsoft/adaptive-apps/main/cli/install.sh | bash
```

```powershell
# Windows PowerShell
iwr -useb https://raw.githubusercontent.com/microsoft/adaptive-apps/main/cli/install.ps1 | iex
```

Override defaults with environment variables:

| Variable          | Scope      | Purpose                                                         |
| ----------------- | ---------- | --------------------------------------------------------------- |
| `ADA_VERSION`     | both       | Pin a release tag (e.g. `cli-v0.1.0`).                          |
| `ADA_HOME`        | both       | Root for bundled artifacts (default `~/.adaptive`).             |
| `ADA_INSTALL_DIR` | shell only | Where the binary is installed (default `/usr/local/bin`).       |
| `ADA_REPO`        | both       | Source GitHub `owner/repo` (default `microsoft/adaptive-apps`). |

On macOS / Linux the shell installer places `ada` into
`/usr/local/bin` (or `$ADA_INSTALL_DIR`) using `sudo` automatically when
required, so it's already on `PATH`. On Windows the PowerShell installer
appends `%USERPROFILE%\.adaptive\bin` to the *User* `PATH` and refreshes
the current session.

The installed tree is:

```text
~/.adaptive/                # Windows: %USERPROFILE%\.adaptive
├── bin/ada                 # binary (ada.exe on Windows; absent on Unix when ADA_INSTALL_DIR is /usr/local/bin)
└── radius/                 # Radius artifacts shipped with the CLI
    ├── app.bicep
    ├── local-env.bicep
    ├── aks-env.bicep
    ├── bicepconfig.json
    ├── types.tgz
    ├── resource-types/types.yaml
    └── recipes/…
```

`ada` resolves `$ADA_HOME` (defaulting to `~/.adaptive`) when it needs
to locate bundled assets. Use `ada radius path --artifact <name>` to
print the resolved path of any bundled artifact — handy for scripting:

```bash
rad resource-type create --from-file "$(ada radius path --artifact types --require-exists)"
rad bicep publish-extension \
  --from-file "$(ada radius path --artifact types)" \
  --target    "$(ada radius path --artifact types-bundle)"
```

Run `ada init` once after installation if you skipped the installer and
want the directories pre-created.

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
