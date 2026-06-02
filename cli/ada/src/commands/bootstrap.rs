//! `ada bootstrap` — install or upgrade a portfolio on the current kube context.
//!
//! Thin orchestrator over `helm upgrade --install`. By default the chart is
//! pulled from the published OCI registry; pass `--chart-root` to install
//! from a local source tree instead.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, ValueEnum};

use crate::ui;

/// Default OCI repository prefix for published portfolio charts.
/// Concrete chart reference is `<prefix>/<portfolio>[-ai]`.
const DEFAULT_CHART_REGISTRY: &str = "oci://ghcr.io/microsoft/adaptive-apps/charts/portfolios";

/// Default chart version pulled when `--version` is not supplied.
const DEFAULT_CHART_VERSION: &str = "0.1.0";

#[derive(Debug, Copy, Clone, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum Portfolio {
    Min,
    Core,
    Ent,
}

impl Portfolio {
    fn as_str(self) -> &'static str {
        match self {
            Portfolio::Min => "min",
            Portfolio::Core => "core",
            Portfolio::Ent => "ent",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum Platform {
    /// Local Kubernetes (kind, k3d, Docker Desktop, minikube).
    Localk8s,
    /// Azure Kubernetes Service.
    Aks,
    /// Azure Arc-enabled Kubernetes.
    Arc,
}

impl Platform {
    fn as_str(self) -> &'static str {
        match self {
            Platform::Localk8s => "localk8s",
            Platform::Aks => "aks",
            Platform::Arc => "arc",
        }
    }
}

/// Additive capability overlays. Today only `ai` exists; more axes
/// (edge, airgap, …) will be added as overlays rather than as new
/// portfolios so the matrix doesn't explode.
#[derive(Debug, Copy, Clone, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum Capability {
    Ai,
}

impl Capability {
    fn as_str(self) -> &'static str {
        match self {
            Capability::Ai => "ai",
        }
    }
}

#[derive(Debug, Args)]
pub struct BootstrapArgs {
    /// Platform portfolio to install.
    #[arg(long, value_enum)]
    portfolio: Portfolio,

    /// Target platform. Selects platform-specific helm overrides and (where
    /// applicable) auto-discovers cluster facts via the cloud CLI.
    ///
    /// - `aks`  — assumes the AKS Istio add-on owns the mesh control plane
    ///           (sets `istio.install.enabled=false`, `istio.namespace=aks-istio-system`).
    ///           If `--azure-subscription` is set, runs `az account set`
    ///           before invoking helm.
    /// - `arc`  — reserved; no automation today.
    /// - `localk8s` — chart installs Istio itself.
    ///
    /// Optional. When omitted no platform overrides are applied and the
    /// install targets whatever cluster the current `kubectl` context
    /// points at.
    #[arg(long, value_enum)]
    platform: Option<Platform>,

    /// Azure subscription to switch to before running helm. Only honored
    /// when `--platform aks`. Runs `az account set --subscription <id>`.
    #[arg(long)]
    azure_subscription: Option<String>,

    /// Azure resource group containing the AKS cluster. Required together
    /// with `--aks-cluster` to auto-discover the active Istio add-on
    /// revision. Only honored when `--platform aks`.
    #[arg(long)]
    resource_group: Option<String>,

    /// AKS cluster name. Required together with `--resource-group` to
    /// auto-discover the active Istio add-on revision. Only honored when
    /// `--platform aks`.
    #[arg(long)]
    aks_cluster: Option<String>,

    /// Namespace where the AKS Istio add-on installs its control plane.
    /// Only honored when `--platform aks`.
    #[arg(long, default_value = "aks-istio-system")]
    aks_istio_namespace: String,

    /// Also install the Radius control plane onto the same cluster after
    /// the chart install succeeds. On `--platform aks` this enables
    /// workload identity (`global.azureWorkloadIdentity.enabled=true`).
    /// Creates a Radius workspace and group (see `--radius-workspace`,
    /// `--radius-group`). Requires `rad` on PATH.
    #[arg(long)]
    with_radius: bool,

    /// Radius workspace name to create / switch to when `--with-radius`
    /// is set.
    #[arg(long, default_value = "adaptive")]
    radius_workspace: String,

    /// Radius group to create when `--with-radius` is set.
    #[arg(long, default_value = "adaptive")]
    radius_group: String,

    /// Skip automatic Azure credential provisioning. By default, when
    /// `--with-radius` is combined with `--platform aks` and both
    /// `--resource-group` and `--aks-cluster` are supplied, `ada` creates
    /// (or reuses) an Entra application named `<cluster>-radius-app`,
    /// federates the four Radius service accounts in `radius-system`,
    /// grants the app `Owner` on the resource group, and registers the
    /// credential with `rad credential register azure wi`. Pass this
    /// flag to skip that automation and register the credential yourself.
    #[arg(long)]
    skip_azure_credentials: bool,

    /// Additive capability overlays (repeatable). e.g. `--with ai`.
    #[arg(long = "with", value_enum, num_args = 0..)]
    with: Vec<Capability>,

    /// Helm release name.
    #[arg(long, default_value = "adaptive-apps")]
    release: String,

    /// Namespace to install into.
    #[arg(long, short = 'n', default_value = "adaptive-apps")]
    namespace: String,

    /// Extra values files appended after the resolved presets (`helm -f`).
    #[arg(long = "values", short = 'f')]
    values: Vec<PathBuf>,

    /// Extra `--set key=value` overrides passed straight to helm.
    #[arg(long = "set")]
    set: Vec<String>,

    /// Print the resolved helm invocation without executing it.
    #[arg(long)]
    dry_run: bool,

    /// Install from a local chart directory instead of the published OCI
    /// registry. Points at the directory that contains the portfolio
    /// charts (legacy layout: `<chart-root>/portfolios/<portfolio>[-ai]`)
    /// or a unified `adaptive-apps/` chart.
    #[arg(long, env = "ADA_CHART_ROOT")]
    chart_root: Option<PathBuf>,

    /// Chart version to pull from the OCI registry. Ignored when
    /// `--chart-root` is set.
    #[arg(long, default_value = DEFAULT_CHART_VERSION)]
    version: String,

    /// OCI repository prefix to pull charts from. The concrete reference
    /// becomes `<chart-registry>/<portfolio>[-ai]`. Ignored when
    /// `--chart-root` is set.
    #[arg(long, default_value = DEFAULT_CHART_REGISTRY, env = "ADA_CHART_REGISTRY")]
    chart_registry: String,

    /// Name of the local k3d cluster to provision/reuse when
    /// `--platform localk8s` is set. The resulting kube context is
    /// `k3d-<name>`.
    #[arg(long, default_value = "localk8s")]
    k3d_cluster: String,

    /// Skip automatic local-cluster provisioning. By default `--platform
    /// localk8s` runs `k3d cluster create <name>` (idempotent) and
    /// switches the kubectl context to `k3d-<name>` before installing.
    #[arg(long)]
    skip_cluster_provision: bool,
}

pub fn run(args: BootstrapArgs) -> Result<()> {
    ensure_tool("helm")?;
    ensure_tool("kubectl")?;

    // Platform automation: switch Azure subscription if requested, derive
    // platform-specific --set overrides. Run before resolve_chart_and_values
    // so failures here surface before we touch the cluster.
    let platform = apply_platform_automation(&args)?;

    let resolution = resolve_chart_and_values(&args)?;

    let mut helm = Command::new("helm");
    helm.arg("upgrade")
        .arg("--install")
        .arg(&args.release)
        .arg(&resolution.chart_ref);
    if let Some(v) = &resolution.version {
        helm.arg("--version").arg(v);
    }
    helm.arg("--namespace")
        .arg(&args.namespace)
        .arg("--create-namespace");

    for v in &resolution.values_files {
        helm.arg("-f").arg(v);
    }
    for v in &args.values {
        helm.arg("-f").arg(v);
    }
    // Platform-derived --set comes BEFORE user --set so the user can override.
    for s in &platform.helm_sets {
        helm.arg("--set").arg(s);
    }
    for s in &args.set {
        helm.arg("--set").arg(s);
    }

    ui::heading("Plan");
    ui::detail("portfolio", args.portfolio.as_str());
    match args.platform {
        Some(p) => ui::detail("platform", p.as_str()),
        None => {
            let current = current_kube_context().unwrap_or_else(|_| "<unknown>".into());
            ui::detail(
                "platform",
                &format!("<auto> (current kubectl context: {current})"),
            );
        }
    }
    if !args.with.is_empty() {
        let with: Vec<&str> = args.with.iter().map(|c| c.as_str()).collect();
        ui::detail("with", &with.join(","));
    }
    ui::detail("release", &args.release);
    ui::detail("namespace", &args.namespace);
    ui::detail("chart", &resolution.chart_ref.to_string_lossy());
    if let Some(v) = &resolution.version {
        ui::detail("version", v);
    }
    if !resolution.values_files.is_empty() {
        ui::bullet("values (resolved):");
        for v in &resolution.values_files {
            ui::bullet(&format!("  - {}", v.display()));
        }
    }
    if !platform.helm_sets.is_empty() {
        ui::bullet("platform overrides:");
        for s in &platform.helm_sets {
            ui::bullet(&format!("  --set {s}"));
        }
    }
    if let Some(rev) = &platform.istio_revision {
        ui::detail("istio rev", &format!("{rev} (discovered)"));
    }
    if args.with_radius {
        ui::detail(
            "with-radius",
            &format!("workspace={} group={}", args.radius_workspace, args.radius_group),
        );
    }
    println!();

    // Provision a local k3d cluster before helm runs on --platform localk8s.
    if matches!(args.platform, Some(Platform::Localk8s)) && !args.skip_cluster_provision {
        provision_k3d_cluster(&args.k3d_cluster, args.dry_run)?;
        println!();
    }

    ui::command(&render_command(&helm));

    if args.dry_run {
        ui::dry_run("not executing helm");
        if args.with_radius {
            install_radius(&args)?;
        }
        return Ok(());
    }

    ui::tool_banner("helm", "upgrade --install");
    let status = helm.status().with_context(|| "failed to spawn helm")?;
    if !status.success() {
        bail!("helm exited with status {status}");
    }
    ui::ok("helm install complete");

    if args.with_radius {
        install_radius(&args)?;
    }

    if let Some(rev) = &platform.istio_revision {
        println!();
        ui::note("AKS Istio add-on revision (use to label app namespaces):");
        println!("export ISTIO_REVISION={rev}");
    }
    Ok(())
}

/// Idempotently provision a local k3d cluster. If a cluster with the
/// requested name already exists we just switch the kubectl context to
/// it; otherwise we create it. Requires `k3d` and `kubectl` on PATH.
fn provision_k3d_cluster(name: &str, dry_run: bool) -> Result<()> {
    ensure_tool("k3d")?;
    let context = format!("k3d-{name}");

    ui::heading("Local cluster");
    ui::detail("k3d cluster", name);
    ui::detail("kube context", &context);

    if dry_run {
        ui::dry_run(&format!(
            "would run: k3d cluster create {name} (if absent), then kubectl config use-context {context}"
        ));
        return Ok(());
    }

    let exists = k3d_cluster_exists(name)?;
    if exists {
        ui::step(&format!("k3d cluster `{name}` already exists, reusing"));
    } else {
        ui::step(&format!("creating k3d cluster `{name}`"));
        let mut cmd = Command::new("k3d");
        cmd.args(["cluster", "create", name]);
        ui::tool_banner("k3d", &render_command(&cmd));
        let status = cmd.status().context("failed to spawn `k3d cluster create`")?;
        if !status.success() {
            bail!("`k3d cluster create {name}` exited with status {status}");
        }
    }

    ui::step(&format!("switching kubectl context to `{context}`"));
    let status = Command::new("kubectl")
        .args(["config", "use-context", &context])
        .status()
        .context("failed to spawn `kubectl config use-context`")?;
    if !status.success() {
        bail!("`kubectl config use-context {context}` failed");
    }
    ui::ok(&format!("local cluster `{name}` ready"));
    Ok(())
}

fn k3d_cluster_exists(name: &str) -> Result<bool> {
    let out = Command::new("k3d")
        .args(["cluster", "list", "--no-headers"])
        .output()
        .context("failed to run `k3d cluster list`")?;
    if !out.status.success() {
        bail!(
            "`k3d cluster list` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .any(|n| n == name))
}

struct Resolution {
    /// Either an OCI ref like `oci://ghcr.io/.../ent` or a local path.
    chart_ref: OsString,
    /// Helm `--version`. Only set for OCI/remote installs.
    version: Option<String>,
    values_files: Vec<PathBuf>,
}

/// Resolve the chart reference and ordered values files for a portfolio +
/// context. When `--chart-root` is set we install from a local source tree;
/// otherwise we pull the published OCI chart at `--version`.
fn resolve_chart_and_values(args: &BootstrapArgs) -> Result<Resolution> {
    if let Some(chart_root) = args.chart_root.as_deref() {
        return resolve_local(chart_root, args);
    }
    resolve_oci(args)
}

fn resolve_oci(args: &BootstrapArgs) -> Result<Resolution> {
    let chart_name = portfolio_chart_name(args);
    let registry = args.chart_registry.trim_end_matches('/');
    let chart_ref = format!("{registry}/{chart_name}");
    Ok(Resolution {
        chart_ref: OsString::from(chart_ref),
        version: Some(args.version.clone()),
        values_files: Vec::new(),
    })
}

fn resolve_local(chart_root: &Path, args: &BootstrapArgs) -> Result<Resolution> {
    let unified_chart = chart_root.join("adaptive-apps");
    if unified_chart.join("Chart.yaml").is_file() {
        return resolve_unified(&unified_chart, args);
    }
    resolve_legacy_per_portfolio(chart_root, args)
}

fn resolve_unified(chart: &Path, args: &BootstrapArgs) -> Result<Resolution> {
    let values_dir = chart.join("values");
    let mut files = Vec::new();

    let portfolio_file = values_dir.join(format!("{}.yaml", args.portfolio.as_str()));
    require_file(&portfolio_file, "portfolio preset")?;
    files.push(portfolio_file);

    for cap in &args.with {
        let overlay = values_dir
            .join("overlays")
            .join(format!("{}.yaml", cap.as_str()));
        require_file(&overlay, "capability overlay")?;
        files.push(overlay);
    }

    if let Some(p) = args.platform {
        let ctx_overlay = values_dir
            .join("overlays")
            .join(format!("platform-{}.yaml", p.as_str()));
        if ctx_overlay.is_file() {
            files.push(ctx_overlay);
        }
    }

    Ok(Resolution {
        chart_ref: chart.as_os_str().to_owned(),
        version: None,
        values_files: files,
    })
}

fn resolve_legacy_per_portfolio(chart_root: &Path, args: &BootstrapArgs) -> Result<Resolution> {
    let chart_name = portfolio_chart_name(args);
    let chart_path = chart_root.join("portfolios").join(&chart_name);
    if !chart_path.join("Chart.yaml").is_file() {
        bail!(
            "no chart at {} (looked for legacy per-portfolio chart; unified chart not present either).\n\
             Pass --chart-root <dir> to point at a different chart-source directory, \
             or omit --chart-root to pull the published chart from {}.",
            chart_path.display(),
            DEFAULT_CHART_REGISTRY
        );
    }
    Ok(Resolution {
        chart_ref: chart_path.into_os_string(),
        version: None,
        values_files: Vec::new(),
    })
}

fn portfolio_chart_name(args: &BootstrapArgs) -> String {
    let has_ai = args.with.iter().any(|c| matches!(c, Capability::Ai));
    if has_ai {
        format!("{}-ai", args.portfolio.as_str())
    } else {
        args.portfolio.as_str().to_string()
    }
}

fn require_file(path: &Path, what: &str) -> Result<()> {
    if !path.is_file() {
        return Err(anyhow!("missing {what}: {}", path.display()));
    }
    Ok(())
}

fn ensure_tool(name: &str) -> Result<()> {
    which::which(name)
        .map(|_| ())
        .map_err(|_| anyhow!("`{name}` not found on PATH; install it and re-run `ada bootstrap`"))
}

/// Result of platform-specific automation: helm `--set` overrides plus any
/// facts discovered about the target cluster.
struct PlatformOutcome {
    helm_sets: Vec<String>,
    /// AKS Istio add-on revision, e.g. `asm-1-24`. Only populated when
    /// `--platform aks` and both `--resource-group` and `--aks-cluster`
    /// are set.
    istio_revision: Option<String>,
}

/// Apply platform-specific automation: switch Azure subscription if
/// requested, derive helm `--set` overrides, discover the Istio revision.
fn apply_platform_automation(args: &BootstrapArgs) -> Result<PlatformOutcome> {
    let Some(platform) = args.platform else {
        if args.azure_subscription.is_some()
            || args.resource_group.is_some()
            || args.aks_cluster.is_some()
        {
            bail!(
                "--azure-subscription / --resource-group / --aks-cluster all require --platform aks"
            );
        }
        return Ok(PlatformOutcome {
            helm_sets: Vec::new(),
            istio_revision: None,
        });
    };

    match platform {
        Platform::Aks => apply_aks_automation(args),
        Platform::Localk8s | Platform::Arc => {
            if args.azure_subscription.is_some()
                || args.resource_group.is_some()
                || args.aks_cluster.is_some()
            {
                bail!(
                    "--azure-subscription / --resource-group / --aks-cluster are only honored with --platform aks"
                );
            }
            Ok(PlatformOutcome {
                helm_sets: Vec::new(),
                istio_revision: None,
            })
        }
    }
}

fn apply_aks_automation(args: &BootstrapArgs) -> Result<PlatformOutcome> {
    if let Some(sub) = &args.azure_subscription {
        ensure_tool("az")?;
        if args.dry_run {
            ui::dry_run(&format!("would run: az account set --subscription {sub}"));
        } else {
            ui::step(&format!("az account set --subscription {sub}"));
            let status = Command::new("az")
                .args(["account", "set", "--subscription"])
                .arg(sub)
                .status()
                .context("failed to run `az account set`")?;
            if !status.success() {
                bail!("`az account set --subscription {sub}` failed");
            }
        }
    }

    let istio_revision = match (&args.resource_group, &args.aks_cluster) {
        (Some(rg), Some(cluster)) => Some(discover_aks_istio_revision(rg, cluster, args.dry_run)?),
        (None, None) => None,
        _ => bail!("--resource-group and --aks-cluster must be supplied together"),
    };

    // AKS owns the Istio control plane via the Istio add-on. The chart
    // must not install its own Istio, and the post-install PeerAuthn
    // hook must target the AKS-managed namespace.
    //
    // The `istio.*` values live in the `core` subchart, so prefix the
    // override path based on where `core` sits in the dependency chain.
    let prefix = istio_values_prefix(args);
    Ok(PlatformOutcome {
        helm_sets: vec![
            format!("{prefix}istio.install.enabled=false"),
            format!("{prefix}istio.namespace={}", args.aks_istio_namespace),
        ],
        istio_revision,
    })
}

/// Return the dotted helm-values prefix needed to address the `core`
/// subchart's `istio.*` values from the top of the chart being installed.
/// Empty string when installing `core` itself.
fn istio_values_prefix(args: &BootstrapArgs) -> &'static str {
    let has_ai = args.with.iter().any(|c| matches!(c, Capability::Ai));
    match (args.portfolio, has_ai) {
        // `core` is the parent chart — istio values are at the top.
        (Portfolio::Core, false) => "",
        // `core-ai` → core subchart.
        (Portfolio::Core, true) => "core.",
        // `ent` → core subchart.
        (Portfolio::Ent, false) => "core.",
        // `ent-ai` → ent subchart → core subchart.
        (Portfolio::Ent, true) => "ent.core.",
        // `min` doesn't bundle istio; sets are still harmless.
        (Portfolio::Min, _) => "",
    }
}

/// Run `az aks show ... --query 'serviceMeshProfile.istio.revisions[0]'`
/// to discover the active Istio add-on revision.
fn discover_aks_istio_revision(
    resource_group: &str,
    cluster: &str,
    dry_run: bool,
) -> Result<String> {
    ensure_tool("az")?;
    if dry_run {
        ui::dry_run(&format!(
            "would run: az aks show -g {resource_group} -n {cluster} --query 'serviceMeshProfile.istio.revisions[0]' -o tsv"
        ));
        return Ok("<discovered-at-apply>".to_string());
    }
    let out = Command::new("az")
        .args([
            "aks",
            "show",
            "-g",
            resource_group,
            "-n",
            cluster,
            "--query",
            "serviceMeshProfile.istio.revisions[0]",
            "-o",
            "tsv",
        ])
        .output()
        .context("failed to run `az aks show`")?;
    if !out.status.success() {
        bail!(
            "az aks show -g {resource_group} -n {cluster} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let rev = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if rev.is_empty() || rev == "null" {
        bail!(
            "AKS cluster {cluster} in resource group {resource_group} has no Istio add-on revision. \
             Enable the add-on with: az aks mesh enable -g {resource_group} -n {cluster}"
        );
    }
    Ok(rev)
}

/// Install the Radius control plane onto the current kube context.
fn install_radius(args: &BootstrapArgs) -> Result<()> {
    ensure_tool("rad")?;

    let mut rad_install = Command::new("rad");
    rad_install.args(["install", "kubernetes"]);
    if matches!(args.platform, Some(Platform::Aks)) {
        rad_install.args(["--set", "global.azureWorkloadIdentity.enabled=true"]);
    }

    let kube_ctx = current_kube_context().unwrap_or_default();
    let mut rad_workspace = Command::new("rad");
    rad_workspace.args(["workspace", "create", "kubernetes", &args.radius_workspace]);
    if !kube_ctx.is_empty() {
        rad_workspace.args(["--context", &kube_ctx]);
    }
    rad_workspace.arg("--force");

    let mut rad_switch = Command::new("rad");
    rad_switch.args(["workspace", "switch", &args.radius_workspace]);

    let mut rad_group = Command::new("rad");
    rad_group.args(["group", "create", &args.radius_group]);

    let steps = [&rad_install, &rad_workspace, &rad_switch, &rad_group];
    println!();
    ui::heading("Radius bootstrap plan:");
    for s in steps {
        ui::command(&render_command(s));
    }

    let azure_creds_planned = wants_azure_credential_automation(args);
    if azure_creds_planned {
        ui::bullet(&format!(
            "+ provision Azure credential for Radius (Entra app `{}-radius-app`, federate radius-system SAs, grant Owner on `{}`, rad credential register azure wi)",
            args.aks_cluster.as_deref().unwrap_or(""),
            args.resource_group.as_deref().unwrap_or(""),
        ));
    }

    if args.dry_run {
        ui::dry_run("not executing Radius install");
        if azure_creds_planned {
            provision_azure_credential(args)?;
        }
        return Ok(());
    }

    for mut cmd in [rad_install, rad_workspace, rad_switch, rad_group] {
        let label = render_command(&cmd);
        ui::tool_banner("rad", &label);
        let status = cmd
            .status()
            .with_context(|| format!("failed to spawn: {label}"))?;
        if !status.success() {
            bail!("command failed ({status}): {label}");
        }
    }
    ui::ok("Radius bootstrap complete");

    if azure_creds_planned {
        provision_azure_credential(args)?;
    }
    Ok(())
}

/// True when we should auto-provision an Azure credential for Radius:
/// `--with-radius` + `--platform aks` + rg + cluster, and not opted out.
fn wants_azure_credential_automation(args: &BootstrapArgs) -> bool {
    args.with_radius
        && !args.skip_azure_credentials
        && matches!(args.platform, Some(Platform::Aks))
        && args.resource_group.is_some()
        && args.aks_cluster.is_some()
}

/// Federated identity subjects for the Radius control plane service
/// accounts in the `radius-system` namespace.
const RADIUS_FEDERATED_SAS: &[(&str, &str)] = &[
    ("radius-applications-rp", "applications-rp"),
    ("radius-bicep-de", "bicep-de"),
    ("radius-ucp", "ucp"),
    ("radius-dynamic-rp", "dynamic-rp"),
];

/// Auto-provision the Azure credential Radius needs to deploy resources.
/// Idempotent: reuses the Entra app and federated credentials if they
/// already exist; the role assignment and `rad credential register` are
/// naturally idempotent.
fn provision_azure_credential(args: &BootstrapArgs) -> Result<()> {
    ensure_tool("az")?;
    ensure_tool("rad")?;

    let rg = args
        .resource_group
        .as_deref()
        .expect("wants_azure_credential_automation guards this");
    let cluster = args
        .aks_cluster
        .as_deref()
        .expect("wants_azure_credential_automation guards this");
    let app_name = format!("{cluster}-radius-app");

    println!();
    ui::heading("Azure credential provisioning for Radius:");
    ui::detail("entra app", &app_name);
    ui::detail(
        "federated",
        &format!(
            "radius-system/{{{}}}",
            RADIUS_FEDERATED_SAS
                .iter()
                .map(|(_, sa)| *sa)
                .collect::<Vec<_>>()
                .join(",")
        ),
    );
    ui::detail("rbac scope", &format!("/subscriptions/<sub>/resourceGroups/{rg} (Owner)"));

    if args.dry_run {
        ui::dry_run("would discover AKS OIDC issuer, create/reuse Entra app, federate SAs, grant Owner, rad credential register azure wi");
        return Ok(());
    }

    let oidc_issuer = az_query(
        &[
            "aks", "show", "-g", rg, "-n", cluster,
            "--query", "oidcIssuerProfile.issuerUrl", "-o", "tsv",
        ],
        "AKS OIDC issuer URL",
    )?;
    if oidc_issuer.is_empty() {
        bail!(
            "AKS cluster {cluster} has no OIDC issuer URL. Enable it with: \
             az aks update -g {rg} -n {cluster} --enable-oidc-issuer --enable-workload-identity"
        );
    }

    let tenant_id = az_query(
        &["account", "show", "--query", "tenantId", "-o", "tsv"],
        "Azure tenant id",
    )?;
    let subscription_id = az_query(
        &["account", "show", "--query", "id", "-o", "tsv"],
        "Azure subscription id",
    )?;

    // Create-or-reuse the Entra application.
    let existing = az_query(
        &[
            "ad", "app", "list", "--display-name", &app_name,
            "--query", "[0].appId", "-o", "tsv",
        ],
        "existing Entra app id",
    )?;
    let app_id = if existing.is_empty() {
        ui::step(&format!("creating Entra application `{app_name}`"));
        az_query(
            &[
                "ad", "app", "create", "--display-name", &app_name,
                "--query", "appId", "-o", "tsv",
            ],
            "new Entra app id",
        )?
    } else {
        ui::step(&format!("reusing existing Entra application `{app_name}` ({existing})"));
        existing
    };
    let app_object_id = az_query(
        &["ad", "app", "show", "--id", &app_id, "--query", "id", "-o", "tsv"],
        "Entra app object id",
    )?;

    // Federate the four Radius service accounts. `az ad app
    // federated-credential create` is not idempotent, so list first.
    let existing_fc = az_query(
        &[
            "ad", "app", "federated-credential", "list",
            "--id", &app_object_id, "--query", "[].name", "-o", "tsv",
        ],
        "existing federated credentials",
    )?;
    let existing_fc: std::collections::HashSet<&str> =
        existing_fc.lines().map(str::trim).filter(|s| !s.is_empty()).collect();

    for (fc_name, sa) in RADIUS_FEDERATED_SAS {
        if existing_fc.contains(fc_name) {
            ui::step(&format!("federated credential `{fc_name}` already exists, skipping"));
            continue;
        }
        let subject = format!("system:serviceaccount:radius-system:{sa}");
        let params = format!(
            r#"{{"name":"{fc_name}","issuer":"{oidc_issuer}","subject":"{subject}","description":"Radius {sa} service account","audiences":["api://AzureADTokenExchange"]}}"#
        );
        ui::step(&format!("creating federated credential `{fc_name}` for {subject}"));
        let status = Command::new("az")
            .args([
                "ad", "app", "federated-credential", "create",
                "--id", &app_object_id,
                "--parameters", &params,
            ])
            .status()
            .context("failed to run `az ad app federated-credential create`")?;
        if !status.success() {
            bail!("failed to create federated credential `{fc_name}`");
        }
    }

    // Ensure service principal exists (idempotent: ignore failure if it
    // already exists).
    ui::step(&format!("ensuring service principal for app `{app_id}`"));
    let _ = Command::new("az")
        .args(["ad", "sp", "create", "--id", &app_id])
        .status();

    // Grant Owner on the resource group.
    let scope = format!("/subscriptions/{subscription_id}/resourceGroups/{rg}");
    ui::step(&format!("granting Owner to `{app_id}` on `{scope}`"));
    let status = Command::new("az")
        .args([
            "role", "assignment", "create",
            "--assignee", &app_id,
            "--role", "Owner",
            "--scope", &scope,
        ])
        .status()
        .context("failed to run `az role assignment create`")?;
    if !status.success() {
        // Role assignment fails if it already exists; that's fine.
        ui::warn("role assignment may already exist; continuing");
    }

    // Register with Radius.
    ui::step(&format!(
        "rad credential register azure wi --client-id {app_id} --tenant-id {tenant_id}"
    ));
    let status = Command::new("rad")
        .args([
            "credential", "register", "azure", "wi",
            "--client-id", &app_id,
            "--tenant-id", &tenant_id,
        ])
        .status()
        .context("failed to run `rad credential register azure wi`")?;
    if !status.success() {
        bail!("`rad credential register azure wi` failed");
    }

    println!();
    ui::note("Azure credential registered. For reference:");
    println!("export APPLICATION_CLIENT_ID={app_id}");
    println!("export TENANT_ID={tenant_id}");
    Ok(())
}

/// Run `az <args>` and return trimmed stdout. Errors include the
/// command's stderr for diagnosis.
fn az_query(az_args: &[&str], what: &str) -> Result<String> {
    let out = Command::new("az")
        .args(az_args)
        .output()
        .with_context(|| format!("failed to run `az` while fetching {what}"))?;
    if !out.status.success() {
        bail!(
            "`az {}` failed while fetching {what}: {}",
            az_args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Read the active kubectl context name (`kubectl config current-context`).
fn current_kube_context() -> Result<String> {
    let out = Command::new("kubectl")
        .args(["config", "current-context"])
        .output()
        .context("failed to run kubectl config current-context")?;
    if !out.status.success() {
        bail!(
            "kubectl config current-context failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn render_command(cmd: &Command) -> String {
    let mut parts = vec![shell_quote(cmd.get_program())];
    for a in cmd.get_args() {
        parts.push(shell_quote(a));
    }
    parts.join(" ")
}

fn shell_quote(s: &OsStr) -> String {
    let s = s.to_string_lossy();
    if s.is_empty() {
        return "''".into();
    }
    let safe = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '=' | ':' | ','));
    if safe {
        s.into_owned()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
