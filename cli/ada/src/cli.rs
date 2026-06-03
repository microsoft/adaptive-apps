use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::{bootstrap, home_cmd, oidc, package};

#[derive(Debug, Parser)]
#[command(
    name = "ada",
    about = "Adaptive Apps CLI",
    long_about = "ada bootstraps and manages adaptive-app platform portfolios \
                  (min, min-ai, core, core-ai, ent, ent-ai) across target \
                  contexts (k3s, k8s, aks, arc, azure-local).",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create `$ADA_HOME` (default `~/.adaptive`) and print the layout.
    Init(home_cmd::InitArgs),

    /// Install or upgrade a portfolio onto the current Kubernetes context.
    Bootstrap(bootstrap::BootstrapArgs),

    /// Analyze an app source folder and emit a Radius app.bicep file.
    Package(package::PackageArgs),

    /// Manage OIDC clients in a chart-deployed Keycloak.
    Oidc(oidc::OidcArgs),

    /// Inspect the bundled Radius artifacts under `$ADA_HOME/radius`.
    Radius(home_cmd::RadiusArgs),
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Init(args) => home_cmd::run_init(args),
            Command::Bootstrap(args) => bootstrap::run(args),
            Command::Package(args) => package::run(args),
            Command::Oidc(args) => oidc::run(args),
            Command::Radius(args) => home_cmd::run_radius(args),
        }
    }
}
