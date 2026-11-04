# `ada` — Adaptive Apps CLI

`ada` is the orchestration entry point for the Adaptive Apps platform. It
wraps `helm`, `kubectl`, and `rad` so installing a portfolio onto a target
context is a single command instead of a tutorial walkthrough.

## Status

The CLI supports `init`, `agents-config`, `bootstrap`, `oidc`, `radius`,
`package` and `skill`. Native setup in this branch uses CLI 0.2.0; build it
locally until that release is published:

```bash
ada bootstrap --portfolio <min|min-ai|core|core-ai|ent|ent-ai> [--platform <k3s|k8s|aks|arc|azure-local>] [--with agents]
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
    ├── agents.example.json
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
oci://ghcr.io/microsoft/adaptive-apps/charts/adaptive-apps
```

at the version selected by `--version` (default `0.2.0`). The portfolio
selection becomes a profile file inside the chart — `profiles/<portfolio>.yaml`.

Pass `--chart-root <dir>` to install from a local chart source tree
instead; ada expects `<chart-root>/adaptive-apps/Chart.yaml` and applies
`<chart-root>/adaptive-apps/profiles/<portfolio>.yaml` automatically.

`--platform` is optional. When omitted no platform overrides are applied and
the install targets whatever cluster the current `kubectl` context points at.
When `--platform aks` is set, ada appends `--set features.istio.install=false`
and `--set istio.namespace=aks-istio-system` so the chart defers to the AKS
Istio add-on; pass `--azure-subscription <id>` to switch subscriptions via
`az account set` before invoking helm.

### Examples

Dry-run the published `ent + ai` chart against the current cluster:

```bash
ada bootstrap --portfolio ent-ai --dry-run
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

### Existing AKS Azure credentials

With `--with-radius --platform aks --resource-group ... --aks-cluster ...`,
bootstrap first checks `rad credential list --workspace <radius-workspace>
-o json`. If an Azure credential is registered, bootstrap preserves it and skips
Entra application discovery, federation, RBAC, and credential registration.
This check does not access Microsoft Graph or verify that the existing
credential still has working federation and Azure permissions. Inspection
failures stop bootstrap rather than triggering provisioning. The check supports
both JSON-only output and the informational banner emitted by Radius CLI 0.57.

The workload-identity helper scripts provision the Entra application and
federation, but do not register the credential with Radius. If you ran a helper
separately, register its application client ID once against the target Radius
workspace before rerunning bootstrap:

```bash
rad credential register azure wi \
  --workspace <radius-workspace> \
  --client-id <application-client-id> \
  --tenant-id <tenant-id>
```

Use the client ID printed by the PowerShell helper when Graph access is blocked
from your deployment workstation. The workspace must already point to the
intended Radius installation. If no credential is registered, automatic
provisioning still requires Graph access. `--skip-azure-credentials` disables
both inspection and provisioning when you manage registration yourself.

### Browser port overrides

For concurrent local and remote native deployments, add
`--keycloak-port 8445 --trading-port 3445 --keep-port-forward` to the remote
bootstrap command. Local ports map to the unchanged Gateway listeners `8444`
and `3443`. Keycloak's public URL, native issuers, and printed OIDC exports
follow the Keycloak override. Trading's callback origin remains controlled by
`tradingOrigin` in the agents configuration.

Set `keycloakPort` in the agents configuration to persist the choice across
`ada init` and bootstrap, or pass `--keycloak-port` to both commands. Without
an override, native Keycloak retains port `8444`; standard bootstrap retains
its HTTP port `8080`. Existing derived inputs require the explicit
`--refresh-derived-state` flag after a reviewed port change. The refresh
preserves retained credentials. Deploy using the newly printed app command
so its issuer and browser endpoint match the new public Keycloak URL.

## Native application setup

Follow [Getting Started](../tutorials/getting-started/README.md) for the shared
CLI/manual flow and [native delivery](../docs/agents/native-radius.md) for
artifact publishing and immutable releases. No AI or session-local helper is
needed to prepare inputs:

```bash
ada agents-config --output agents.json --import-images
ada init --agents-config agents.json --namespace min-ai --release min-ai
ada bootstrap --portfolio min-ai --platform k8s --release min-ai --namespace min-ai \
  --chart-root charts --with-radius --with agents --agents-config agents.json --demo-users
ada oidc demo-users --namespace min-ai --release min-ai
```

`ada agents-config` creates `agents.json` from the bundled
`radius/agents.example.json`, the origin GitHub repository and its newest
published immutable `recipes-<commit>` release containing `recipes.tgz`.
Pass `--repository OWNER/REPOSITORY`, `--commit FULL_SHA` or `--template PATH`
to override those inputs. An explicit commit must already have that release;
the command fails before writing configuration otherwise. Running it again
updates only the archive and registry locations, preserving reviewed models,
explicit image digests and identity settings.
New configurations contain no model declarations by default. Pass
`--with-kaito-model` only after installing and validating KAITO/GPU support;
it opts a new configuration into the pinned Qwen3-0.6B example.
To use public OpenAI consistently for the trading application and native
agents, pass `--with-openai-model --openai-api-key-file PATH`, plus optional
`--openai-model` and `--openai-endpoint` overrides. The generated configuration
selects `openai-chat`; `ada init` retains the key and derives both the Model
Gateway Secret and the private Radius application parameter file. The emitted
deployment command contains no API key and accepts no separate native
`AI_PARAMS` override.

`--import-images` runs `sudo k3s ctr -n k8s.io images pull` for required images
that are not already present. It preserves configured immutable `images`
references and resolves missing components from `imageRegistry`/`imageTag`,
then persists the complete immutable digest map in `agents.json`. This also
upgrades older maps when a required component is added. Subsequent `ada init`
runs no longer depend on mutable tags remaining in local K3s.
`agentModel` selects the Model Gateway name printed for the application
deployment. `agentNames` controls the explicit gateway client allowlist;
`peers` contains actual retained `keycloakClientId`, `agentId` and `targets`.
For connected provisioning,
`connected` requires `tenantId`, `provisionerClientId`, `sponsorUserId`,
`ownerUserId`, `oidcIssuer`, `certificateFile` and `certificatePasswordFile`.
Supply reviewed credentials; the CLI never invents tenant/issuer IDs or uses
an Azure CLI Graph bearer. Human Entra federation is a separate reviewed login
declaration.

`models` contains explicit `{name, properties}` Radius model declarations
(`localWorkspace`, `localService`, `azure` or captured `members`). Pass
`ada agents-config --without-models` to clear model declarations from an
existing configuration. Add reviewed model declarations and refresh derived
state later. KAITO is never implied by selecting an `*-ai` portfolio, and no
CPU server is generated.
Foundry models require a separate `modelGatewayIdentity` with reviewed
`clientId` and `tenantId`. Configure its exact cluster issuer, namespace
ServiceAccount federation and scoped inference role before use; this identity
is independent of `connected` Graph/agent-blueprint provisioning.
Gateway mounts and rollout input digests are derived from the declared models.

Project `--state-dir` defaults to `.adaptive`, **not** `$ADA_HOME`.
`init`/native `bootstrap` generate missing secrets, a checked localhost TLS pair,
secure Radius parameters, `portfolio.values.json` and `secrets.json` under
`private/`. Native bootstrap prints the complete second-terminal export and
`rad deploy radius/app.bicep` block; `--keep-port-forward` keeps its HTTPS
ingress available on `localhost:8444`. Demo seeding saves `demo-users.json`
before creating users, assigns
only the API client's `agent.invoke` role, and never resets an existing password.
Generated files use mode 0600 and the private directory mode 0700 on Unix.
Unchanged setup is repeatable; mismatching inputs fail rather than overwrite
existing credentials or declarations. After reviewing an intentional config,
namespace, release, or generated-schema change, rerun `ada init` with
`--refresh-derived-state`. This replaces only derived JSON under `private/`;
retained credentials and the TLS key pair are reused unchanged. Pass the same
flag to `ada bootstrap` when bootstrap is preparing the changed inputs directly.

The current kube context remains authoritative. `access/` is optional local
kubeconfig/Radius isolation selected by the user, not regenerated by `init`;
`diagnostics/` contains actual validation output. Windows additionally requires
appropriate private-directory ACLs. OpenSSL, imported/pullable image digests,
published recipes, Envoy-compatible Kubernetes and enforced NetworkPolicies
are native prerequisites.

Native UI clients remain Terraform-owned. To use the trading frontend on the
same portfolio, provision its separate confidential master-realm client with
`ada oidc setup-client --namespace min-ai --release min-ai
--admin-password-file .adaptive/private/keycloak-admin-password`.

## Roadmap

- `ada bootstrap` ✅ (this PR)
- `ada doctor` — prereq + cluster sanity checks
- `ada diff --from <p1> --to <p2>` — values + rendered manifest diff
- `ada explain --portfolio <p>` — human description of what's enabled
- `ada upgrade` / `ada teardown`

Tracked alongside the chart-unification work; see the parent repo discussion.
