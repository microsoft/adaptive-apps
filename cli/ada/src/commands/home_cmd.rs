//! `ada init` and `ada radius` — surface the `$ADA_HOME` layout so
//! install scripts, tutorials, and ad-hoc shells can locate the
//! Radius artifacts that ship alongside the binary.

use anyhow::{bail, Result};
use clap::{Args, Subcommand, ValueEnum};

use crate::home;
use crate::ui;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Print the resolved layout without creating any directories.
    #[arg(long)]
    dry_run: bool,
}

pub fn run_init(args: InitArgs) -> Result<()> {
    let root = home::root()?;
    ui::heading("ada init");
    ui::detail("ada home", &root.display().to_string());
    ui::detail("bin dir", &root.join("bin").display().to_string());
    ui::detail("radius dir", &root.join("radius").display().to_string());

    if args.dry_run {
        ui::dry_run("not creating directories");
        return Ok(());
    }

    let created = home::ensure()?;
    ui::ok(&format!("ensured {}", created.display()));
    Ok(())
}

#[derive(Debug, Args)]
pub struct RadiusArgs {
    #[command(subcommand)]
    command: RadiusCommand,
}

#[derive(Debug, Subcommand)]
enum RadiusCommand {
    /// Print the on-disk path to a bundled Radius artifact.
    Path(RadiusPathArgs),
}

#[derive(Debug, Args)]
pub struct RadiusPathArgs {
    /// Which artifact to resolve.
    #[arg(long, value_enum, default_value_t = RadiusArtifact::Root)]
    artifact: RadiusArtifact,

    /// Require the resolved path to actually exist on disk.
    #[arg(long)]
    require_exists: bool,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
#[value(rename_all = "kebab-case")]
enum RadiusArtifact {
    /// `$ADA_HOME/radius`
    Root,
    /// `$ADA_HOME/radius/bicepconfig.json`
    Bicepconfig,
    /// `$ADA_HOME/radius/resource-types/types.yaml`
    Types,
    /// `$ADA_HOME/radius/types.tgz`
    TypesBundle,
    /// `$ADA_HOME/radius/recipes`
    Recipes,
    /// `$ADA_HOME/radius/local-env.bicep`
    LocalEnv,
    /// `$ADA_HOME/radius/aks-env.bicep`
    AksEnv,
    /// `$ADA_HOME/radius/app.bicep`
    App,
}

pub fn run_radius(args: RadiusArgs) -> Result<()> {
    match args.command {
        RadiusCommand::Path(p) => run_radius_path(p),
    }
}

fn run_radius_path(args: RadiusPathArgs) -> Result<()> {
    let radius_root = home::radius_root()?;
    let path = match args.artifact {
        RadiusArtifact::Root => radius_root.clone(),
        RadiusArtifact::Bicepconfig => radius_root.join("bicepconfig.json"),
        RadiusArtifact::Types => radius_root.join("resource-types").join("types.yaml"),
        RadiusArtifact::TypesBundle => radius_root.join("types.tgz"),
        RadiusArtifact::Recipes => radius_root.join("recipes"),
        RadiusArtifact::LocalEnv => radius_root.join("local-env.bicep"),
        RadiusArtifact::AksEnv => radius_root.join("aks-env.bicep"),
        RadiusArtifact::App => radius_root.join("app.bicep"),
    };
    if args.require_exists && !path.exists() {
        bail!(
            "{} does not exist — run `ada init` and verify the install (expected under $ADA_HOME)",
            path.display()
        );
    }
    println!("{}", path.display());
    Ok(())
}
