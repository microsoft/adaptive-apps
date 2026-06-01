use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::bootstrap;

#[derive(Debug, Parser)]
#[command(
    name = "ada",
    about = "Adaptive Apps CLI",
    long_about = "ada bootstraps and manages portable-app platform portfolios \
                  (min, core, ent) across target contexts (localk8s, aks, arc).",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Install or upgrade a portfolio onto the current Kubernetes context.
    Bootstrap(bootstrap::BootstrapArgs),
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Bootstrap(args) => bootstrap::run(args),
        }
    }
}
