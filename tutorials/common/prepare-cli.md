# Prepare the Adaptive Apps CLI (`ada`)

`ada` is the orchestration entry point for Adaptive Apps. It wraps `helm`,
`kubectl`, and `rad` so installing a portfolio, packaging an app, or
provisioning an OIDC client is one command instead of a tutorial
walkthrough.

This page covers the two supported ways to get `ada` onto your machine:

* [Option A — install a published release](#option-a--install-a-published-release) (recommended for tutorials and demos).
* [Option B — build and run from source](#option-b--build-and-run-from-source) (recommended when iterating on the CLI itself).

Either path lays the same on-disk layout out under `$ADA_HOME` (defaults
to `~/.adaptive`):

```text
~/.adaptive/
├── bin/ada              # CLI binary on Windows (installer also adds bin/ to User PATH)
└── radius/              # Radius artifacts shipped with the CLI
    ├── app.bicep
    ├── local-env.bicep
    ├── aks-env.bicep
    ├── bicepconfig.json
    ├── types.tgz
    ├── resource-types/types.yaml
    └── recipes/…
```

> On macOS / Linux the shell installer drops `ada` into
> `/usr/local/bin/ada` (or the directory you set via `ADA_INSTALL_DIR`)
> so it's already on `PATH`. Only the bundled `radius/` tree lives under
> `$ADA_HOME` on those platforms.

`ada` resolves bundled Radius assets relative to `$ADA_HOME/radius`,
so the rest of the tutorials can reference them by name rather than by
path.

---

## Option A — install a published release

Each tagged `cli-vX.Y.Z` release ships a per-platform archive containing
the `ada` binary plus the matching `radius/` asset tree. The installer
scripts download the right archive for your OS/arch, place `ada` on
your `PATH`, and stage the Radius artifacts under `$ADA_HOME`.

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/microsoft/adaptive-apps/main/cli/install.sh | bash
```

The binary is installed into `/usr/local/bin/ada` by default (using
`sudo` automatically when needed). Override with `ADA_INSTALL_DIR` —
for a no-sudo install:

```bash
ADA_INSTALL_DIR="$HOME/.local/bin" \
  bash -c "$(curl -fsSL https://raw.githubusercontent.com/microsoft/adaptive-apps/main/cli/install.sh)"
```

Then make sure your chosen install dir is on `PATH` (most distros
already include `/usr/local/bin` and `~/.local/bin`).

### Windows (PowerShell)

```powershell
iwr -useb https://raw.githubusercontent.com/microsoft/adaptive-apps/main/cli/install.ps1 | iex
```

The binary is installed into `%USERPROFILE%\.adaptive\bin\ada.exe` and
that directory is appended to the **User** `PATH` automatically (the
current session is refreshed too, so `ada` works right away). Pass
`-SkipPathUpdate` to opt out.

### Installer environment variables

| Variable          | Scope        | Purpose                                                            |
| ----------------- | ------------ | ------------------------------------------------------------------ |
| `ADA_VERSION`     | both         | Pin a release tag (e.g. `cli-v0.1.0`). Defaults to latest `cli-v*`.|
| `ADA_HOME`        | both         | Root for bundled artifacts. Defaults to `~/.adaptive`.             |
| `ADA_INSTALL_DIR` | shell only   | Where to drop the `ada` binary. Defaults to `/usr/local/bin`.      |
| `ADA_REPO`        | both         | Source repo (`owner/repo`). Defaults to `microsoft/adaptive-apps`. |

Example — install a specific version into a custom location without sudo:

```bash
ADA_HOME="$HOME/tools/adaptive" \
ADA_INSTALL_DIR="$HOME/.local/bin" \
ADA_VERSION=cli-v0.1.0 \
  bash -c "$(curl -fsSL https://raw.githubusercontent.com/microsoft/adaptive-apps/main/cli/install.sh)"
```

### Verify the install

```bash
ada --version
ada radius path --artifact root --require-exists
```

The second command prints the resolved Radius asset directory and exits
non-zero if anything is missing — a quick sanity check before running
the rest of the tutorial.

---

## Option B — build and run from source

Use this path when you're modifying the CLI or want to track `main`.

### Prerequisites

* [Rust toolchain](https://www.rust-lang.org/tools/install) (`rustup`, stable, 1.80+). The repo pins `rust-version = "1.80"` in `cli/Cargo.toml`.
* `git`.

### Clone and build

```bash
git clone https://github.com/microsoft/adaptive-apps.git
cd adaptive-apps/cli
cargo build --release
./target/release/ada --version
```

### Install the binary onto `PATH` and stage Radius artifacts

The CLI looks for Radius assets under `$ADA_HOME/radius`, and the
binary needs to be on `PATH`. Mirror the published installer's layout:

```bash
# from the repo root — installs ada to /usr/local/bin (uses sudo automatically)
sudo install -m 0755 cli/target/release/ada /usr/local/bin/ada

# stage the in-repo Radius tree under ~/.adaptive/radius
mkdir -p "$HOME/.adaptive/radius"
cp -R radius/. "$HOME/.adaptive/radius/"

ada --version
ada radius path --artifact root --require-exists
```

Prefer a no-sudo install? Pick any directory already on your `PATH` —
`~/.local/bin` is on most modern distros:

```bash
mkdir -p "$HOME/.local/bin"
install -m 0755 cli/target/release/ada "$HOME/.local/bin/ada"

# add ~/.local/bin to PATH if it isn't already
case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *)
  echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc   # or ~/.zshrc
  export PATH="$HOME/.local/bin:$PATH"
;; esac
```

### Run without installing

You can also point `ada` at the in-repo Radius tree without copying
anything by setting `ADA_HOME`, or by running directly via `cargo run`:

```bash
# in-repo radius/ tree, no system install
ADA_HOME="$PWD" ./cli/target/release/ada radius path --artifact root

# or via cargo run from the workspace
cd cli
ADA_HOME="$PWD/.." cargo run -- radius path --artifact root
```

`ada` only reads under `$ADA_HOME/radius`, so a symlink is enough for
ad-hoc dev loops:

```bash
mkdir -p /tmp/ada-dev
ln -sfn "$PWD/radius" /tmp/ada-dev/radius
ADA_HOME=/tmp/ada-dev ./cli/target/release/ada radius path --artifact types --require-exists
```

### Rebuild after pulling

```bash
cd adaptive-apps
git pull
cargo build --release --manifest-path cli/Cargo.toml
sudo install -m 0755 cli/target/release/ada /usr/local/bin/ada
cp -R radius/. "$HOME/.adaptive/radius/"
```

---

## Next steps

* `ada init` — (re)create the `$ADA_HOME` layout.
* `ada radius path --artifact <root|bicepconfig|types|types-bundle|recipes|local-env|aks-env|app>` — resolve a bundled artifact for scripting.
* `ada bootstrap --portfolio <min|core|ent> [--with ai] [--platform <localk8s|aks|arc>]` — install a portfolio onto the current `kubectl` context.
* `ada oidc setup-client …` — provision an OIDC client in a chart-deployed Keycloak.
* `ada package -i <src> -o app.bicep [--llm]` — analyze a `docker-compose.yml` and emit a Radius `app.bicep`.

Run `ada <command> --help` for the full flag list of any subcommand.
