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

use crate::commands::{
    agents,
    oidc::{self, DemoUsersArgs, SetupClientArgs},
};
use crate::ui;

/// Default OCI repository prefix for the published `adaptive-apps` chart.
/// Concrete chart reference is `<prefix>/adaptive-apps`.
const DEFAULT_CHART_REGISTRY: &str = "oci://ghcr.io/microsoft/adaptive-apps/charts";

/// Default chart version pulled when `--version` is not supplied.
const DEFAULT_CHART_VERSION: &str = "0.3.2";

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Portfolio {
    Min,
    MinAi,
    Core,
    CoreAi,
    Ent,
    EntAi,
}

impl Portfolio {
    /// Full slug used as the chart name and profile filename
    /// (e.g. `core`, `core-ai`).
    fn as_str(self) -> &'static str {
        match self {
            Portfolio::Min => "min",
            Portfolio::MinAi => "min-ai",
            Portfolio::Core => "core",
            Portfolio::CoreAi => "core-ai",
            Portfolio::Ent => "ent",
            Portfolio::EntAi => "ent-ai",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Platform {
    /// Local k3s via k3d. `ada bootstrap` will auto-provision a cluster
    /// (`k3d cluster create <name>`) and switch the kubectl context to it.
    /// Pass `--skip-cluster-provision` to reuse an existing k3d cluster.
    K3s,
    /// Bring-your-own standard Kubernetes distribution (kind, Docker
    /// Desktop, minikube, vanilla k8s, etc.). Cluster must already exist;
    /// `ada` runs helm/kubectl/rad against whatever cluster the current
    /// kubectl context points at — no provisioning, no platform overrides.
    K8s,
    /// Azure Kubernetes Service. Cluster must already exist; `ada` discovers
    /// the Istio add-on revision and applies the matching helm overrides.
    Aks,
    /// Azure Arc-enabled Kubernetes (on-prem, edge, other clouds). Cluster
    /// must already exist and be Arc-connected; `ada` does no cloud-side
    /// automation — just runs helm/kubectl/rad against the current context.
    Arc,
    /// Azure Local (formerly Azure Stack HCI). Cluster must already exist;
    /// `ada` does no cloud-side automation — just runs helm/kubectl/rad
    /// against the current context.
    AzureLocal,
}

impl Platform {
    fn as_str(self) -> &'static str {
        match self {
            Platform::K3s => "k3s",
            Platform::K8s => "k8s",
            Platform::Aks => "aks",
            Platform::Arc => "arc",
            Platform::AzureLocal => "azure-local",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Capability {
    Agents,
}

#[derive(Debug, Args)]
pub struct BootstrapArgs {
    /// Platform portfolio to install.
    #[arg(long, value_enum)]
    portfolio: Portfolio,

    /// Target platform. Selects platform-specific helm overrides and (where
    /// applicable) auto-discovers cluster facts via the cloud CLI.
    ///
    /// - `k3s` — chart installs Istio itself. `ada bootstrap` will
    ///           auto-provision a local k3d cluster (pass
    ///           `--skip-cluster-provision` to use an existing one).
    /// - `k8s` — bring-your-own standard Kubernetes distribution. No
    ///           auto-provisioning, no platform overrides; `ada` just runs
    ///           helm/kubectl/rad against the current `kubectl` context.
    /// - `aks` — assumes the AKS Istio add-on owns the mesh control plane
    ///           (sets `istio.install.enabled=false`,
    ///           `istio.namespace=aks-istio-system`). If
    ///           `--azure-subscription` is set, runs `az account set`
    ///           before invoking helm. Cluster must already exist.
    /// - `arc`, `azure-local` — no automation; `ada` just runs
    ///           helm/kubectl/rad against whatever cluster the current
    ///           `kubectl` context points at. Use these when you've
    ///           provisioned the cluster yourself with the standard
    ///           Kubernetes toolchain.
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

    /// Radius environment name to create (must match the environment
    /// resource name in the env bicep file, e.g. `trading` in
    /// `local-env.bicep`).
    #[arg(long, default_value = "trading")]
    radius_environment: String,

    /// Directory containing `resource-types/types.yaml` and the
    /// environment bicep files (`local-env.bicep`, `aks-env.bicep`).
    /// Used by `--with-radius` to register the custom resource types
    /// and deploy the Radius Environment + recipes. Defaults to
    /// `radius` relative to the current working directory.
    #[arg(long, default_value = "radius")]
    radius_dir: PathBuf,

    /// Skip registering the project's Radius resource types and
    /// deploying the environment bicep when `--with-radius` is set.
    /// Pass this flag if you want to deploy the env file manually.
    #[arg(long)]
    skip_radius_recipes: bool,

    /// Skip automatic Azure credential provisioning. By default, when
    /// `--with-radius` is combined with `--platform aks` and both
    /// `--resource-group` and `--aks-cluster` are supplied, `ada` first
    /// checks for an Azure credential registered with Radius and reuses it
    /// without querying Microsoft Graph. Otherwise it creates
    /// (or reuses) an Entra application named `<cluster>-radius-app`,
    /// federates the four Radius service accounts in `radius-system`,
    /// grants the app `Owner` on the resource group, and registers the
    /// credential with `rad credential register azure wi`. Pass this
    /// flag to skip that automation and register the credential yourself.
    #[arg(long)]
    skip_azure_credentials: bool,

    /// Add the native prompt/hosted agent application capability.
    #[arg(long, value_enum)]
    with: Vec<Capability>,

    /// Reviewed native recipe/archive and immutable image configuration.
    #[arg(long, requires = "with")]
    agents_config: Option<PathBuf>,

    /// Project-local private inputs; distinct from $ADA_HOME.
    #[arg(long, default_value = ".adaptive")]
    state_dir: PathBuf,

    /// Replace derived portable-agent JSON state after reviewing changed inputs.
    #[arg(long, requires = "agents_config")]
    refresh_derived_state: bool,

    /// Seed alice/bob with the native API client invocation role.
    #[arg(long, requires = "with")]
    demo_users: bool,

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
    /// registry. Points at the directory that contains the
    /// `adaptive-apps/` chart (i.e. so that
    /// `<chart-root>/adaptive-apps/Chart.yaml` exists).
    #[arg(long, env = "ADA_CHART_ROOT")]
    chart_root: Option<PathBuf>,

    /// Chart version to pull from the OCI registry. Ignored when
    /// `--chart-root` is set.
    #[arg(long, default_value = DEFAULT_CHART_VERSION)]
    version: String,

    /// OCI repository prefix to pull the chart from. The concrete
    /// reference becomes `<chart-registry>/adaptive-apps`. Ignored when
    /// `--chart-root` is set.
    #[arg(long, default_value = DEFAULT_CHART_REGISTRY, env = "ADA_CHART_REGISTRY")]
    chart_registry: String,

    /// Name of the local k3d cluster to provision/reuse when
    /// `--platform k3s` is set. The resulting kube context is
    /// `k3d-<name>`.
    #[arg(long, default_value = "k3s")]
    k3d_cluster: String,

    /// Skip automatic local-cluster provisioning. By default `--platform
    /// k3s` runs `k3d cluster create <name>` (idempotent) and
    /// switches the kubectl context to `k3d-<name>` before installing.
    #[arg(long)]
    skip_cluster_provision: bool,

    /// Skip automatic Keycloak OIDC client provisioning. By default, after
    /// helm install succeeds, `ada` connects to the chart-deployed
    /// Keycloak via port-forward and creates (or reuses) an OIDC client
    /// so the operator doesn't have to do it through the admin console.
    /// Pass this flag to skip and provision the client manually.
    #[arg(long)]
    skip_oidc_client: bool,

    /// OIDC clientId to provision in Keycloak.
    #[arg(long, default_value = "adaptive-apps")]
    oidc_client_id: String,

    /// Valid redirect URI for the OIDC client (repeatable).
    #[arg(long = "oidc-redirect-uri", default_values_t = vec!["http://localhost:3000/*".to_string()])]
    oidc_redirect_uris: Vec<String>,

    /// Keycloak realm hosting the OIDC client.
    #[arg(long, default_value = "master")]
    oidc_realm: String,

    /// Keycloak admin username (defaults to the chart's default).
    #[arg(long, default_value = "admin")]
    keycloak_admin_user: String,

    /// Keycloak admin password (defaults to the chart's default).
    #[arg(long, default_value = "admin")]
    keycloak_admin_password: String,

    /// Browser-facing Keycloak port. Defaults to 8444 with agents (or the
    /// agents config's keycloakPort), and 8080 otherwise. Native mode also
    /// updates Keycloak's public URL, OIDC issuers, and generated app exports.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    keycloak_port: Option<u16>,

    /// Local HTTPS trading Gateway port with agents. Does not change the
    /// frontend callback origin; set tradingOrigin in the agents config too.
    #[arg(long, requires = "with", value_parser = clap::value_parser!(u16).range(1..))]
    trading_port: Option<u16>,

    /// After provisioning the OIDC client, keep the `kubectl port-forward`
    /// to Keycloak running in the foreground (press Ctrl-C to stop).
    /// Use this when you want to run `rad deploy` from another terminal
    /// against localhost without setting up port-forwarding yourself.
    /// Use --keycloak-port (and --trading-port with agents) to avoid
    /// conflicts with another deployment.
    #[arg(long)]
    keep_port_forward: bool,
}

pub fn run(args: BootstrapArgs) -> Result<()> {
    ensure_tool("helm")?;
    ensure_tool("kubectl")?;
    let native = args.with.contains(&Capability::Agents);
    if native && (args.agents_config.is_none() || !args.with_radius || args.skip_radius_recipes) {
        bail!("--with agents requires --agents-config and --with-radius without --skip-radius-recipes");
    }
    if native
        && matches!(
            args.portfolio,
            Portfolio::Min | Portfolio::Core | Portfolio::Ent
        )
    {
        bail!("Select an AI portfolio for the portable-agent capability");
    }

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
    if native {
        helm.arg("-f")
            .arg(args.state_dir.join("private/portfolio.values.json"));
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
    if native {
        ui::detail("capability", "agents (shared Keycloak and telemetry)");
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
            &format!(
                "workspace={} group={}",
                args.radius_workspace, args.radius_group
            ),
        );
    }
    println!();

    // Provision a local k3d cluster before helm runs on --platform k3s.
    if matches!(args.platform, Some(Platform::K3s)) && !args.skip_cluster_provision {
        provision_k3d_cluster(&args.k3d_cluster, args.dry_run, native)?;
        println!();
    }

    if args.chart_root.is_some() {
        build_local_chart_dependencies(Path::new(&resolution.chart_ref), args.dry_run)?;
    }

    ui::command(&render_command(&helm));

    if args.dry_run {
        ui::dry_run("not executing helm");
        if native {
            ui::dry_run("would prepare private inputs, install Envoy v1.9.2 and agentgateway v1.6.0, wait for shared Keycloak, deploy portable-agent services and optionally seed demo users");
        }
        if args.with_radius {
            install_radius(&args)?;
            print_app_deploy_hint(&args);
        }
        maybe_provision_oidc_client(&args)?;
        return Ok(());
    }

    let prepared = if native {
        agents::check_ownership(&args.namespace, &args.release)?;
        let prepared = agents::prepare(
            args.agents_config
                .as_deref()
                .context("Missing native config")?,
            &args.state_dir,
            &args.namespace,
            &args.release,
            &args.radius_group,
            args.refresh_derived_state,
            args.keycloak_port,
        )?;
        native_browser_ports(args.trading_port, prepared.keycloak_port)?;
        agents::save_derived(
            &prepared.private.join("trading.parameters.json"),
            &serde_json::json!({
                "parameters": {"enableIstioInjection": {"value": !matches!(args.portfolio, Portfolio::Min | Portfolio::MinAi)}}
            }),
            args.refresh_derived_state,
        )?;
        install_native_gateways(&args.namespace)?;
        install_radius(&args)?;
        native_prerequisites(&args, &prepared)?;
        Some(prepared)
    } else {
        None
    };
    ui::tool_banner("helm", "upgrade --install");
    let status = helm.status().with_context(|| "failed to spawn helm")?;
    if !status.success() {
        bail!("helm exited with status {status}");
    }
    ui::ok("helm install complete");

    if args.with_radius && !native {
        install_radius(&args)?;
        print_app_deploy_hint(&args);
    }

    let mut native_port_forward = None;
    if let Some(prepared) = prepared {
        for (file, parameters) in [
            ("agents-env.bicep", "environment.parameters.json"),
            ("credentials.bicep", "credentials.parameters.json"),
            ("agent-services.bicep", "services.parameters.json"),
        ] {
            if file == "agent-services.bicep" {
                oidc::wait_native_keycloak(&args.namespace, &args.release)?;
                oidc::provision_native_trading_client(
                    &args.namespace,
                    &args.release,
                    &args.state_dir,
                )?;
            }
            agents::deploy(
                &args.radius_dir,
                file,
                &prepared,
                &args.radius_group,
                parameters,
            )?;
        }
        let status = Command::new("kubectl")
            .args([
                "-n",
                &args.namespace,
                "rollout",
                "restart",
                "deployment/model-gateway",
            ])
            .status()?;
        if !status.success() {
            bail!("Model Gateway restart failed after model configuration deployment");
        }
        let status = Command::new("kubectl")
            .args([
                "-n",
                &args.namespace,
                "rollout",
                "status",
                "deployment",
                "--timeout=15m",
            ])
            .status()?;
        if !status.success() {
            bail!("Native shared-service rollout did not become ready");
        }
        if args.demo_users {
            oidc::provision_demo_users(&DemoUsersArgs {
                namespace: args.namespace.clone(),
                release: args.release.clone(),
                state_dir: args.state_dir.clone(),
                local_port: 18080,
            })?;
        }
        print_native_app_exports(&args, &prepared)?;
        if args.keep_port_forward {
            let service = native_ui_gateway_service(&args.namespace)?;
            native_port_forward = Some(oidc::PortForward::start_service_ports(
                "envoy-gateway-system",
                &service,
                &native_browser_ports(args.trading_port, prepared.keycloak_port)?,
            )?);
        }
    } else {
        maybe_provision_oidc_client(&args)?;
    }

    if let Some(rev) = &platform.istio_revision {
        println!();
        ui::note("AKS Istio add-on revision (use to label app namespaces):");
        println!("export ISTIO_REVISION={rev}");
    }
    if let Some(mut pf) = native_port_forward {
        println!();
        ui::note("keeping trading and identity HTTPS ingress on the ports shown above — press Ctrl-C to stop");
        ui::note("copy the exports and `rad deploy` command above into another terminal");
        let _ = pf.wait();
    }
    Ok(())
}

fn print_native_app_exports(args: &BootstrapArgs, prepared: &agents::Prepared) -> Result<()> {
    let client_secret =
        std::fs::read_to_string(prepared.private.join("adaptive-trading-client-secret"))?;
    let keycloak = format!(
        "http://{}-keycloak.{}.svc.cluster.local:8080",
        args.release, args.namespace
    );
    println!();
    ui::note("Native services ready. In a second terminal, run:");
    println!("export AUTH_USERNAME=admin");
    println!("export AUTH_PASSWORD=admin");
    println!("export OIDC_CLIENT_ID=adaptive-trading");
    println!("export OIDC_CLIENT_SECRET={}", client_secret.trim());
    let issuer = format!(
        "https://localhost:{}/realms/adaptive",
        prepared.keycloak_port
    );
    println!("export OIDC_ISSUER={issuer}");
    println!("export OIDC_AUTH_ENDPOINT={keycloak}/realms/adaptive/protocol/openid-connect/auth");
    println!("export OIDC_BROWSER_AUTH_ENDPOINT={issuer}/protocol/openid-connect/auth");
    println!("export OIDC_TOKEN_ENDPOINT={keycloak}/realms/adaptive/protocol/openid-connect/token");
    println!(
        "export OIDC_USERINFO_ENDPOINT={keycloak}/realms/adaptive/protocol/openid-connect/userinfo"
    );
    println!("export AGENT_ENVIRONMENT={}", prepared.environment);
    println!(
        "export AGENT_GATEWAY_URL=http://agent-ingress.{}.svc.cluster.local:8080",
        args.namespace
    );
    println!("export NATIVE_MODEL={}", prepared.agent_model);
    println!("export PROMPT_AGENT_IMAGE={}", prepared.prompt_agent_image);
    println!("export EDITOR_IMAGE={}", prepared.editor_image);
    println!("export NATIVE_UI_IMAGE={}", prepared.embedded_ui_image);
    println!(
        "export TRADING_IMAGE_REGISTRY={}",
        shell_quote(OsStr::new(&prepared.image_registry))
    );
    println!(
        "export TRADING_IMAGE_TAG={}",
        shell_quote(OsStr::new(&prepared.image_tag))
    );
    println!();
    println!("rad deploy radius/app.bicep \\");
    println!("  --group {} \\", args.radius_group);
    println!("  --environment {} \\", args.radius_environment);
    println!("  --parameters imageRegistry=\"$TRADING_IMAGE_REGISTRY\" \\");
    println!("  --parameters imageTag=\"$TRADING_IMAGE_TAG\" \\");
    if matches!(args.platform, Some(Platform::K3s)) {
        println!("  --parameters computePlatform=k3s \\");
    }
    println!("  --parameters authUsername=\"$AUTH_USERNAME\" \\");
    println!("  --parameters authPassword=\"$AUTH_PASSWORD\" \\");
    println!(
        "  --parameters otelCollectorEndpoint=\"http://otel-collector.{}:4318\" \\",
        args.namespace
    );
    println!("  --parameters oidcIssuer=\"$OIDC_ISSUER\" \\");
    println!("  --parameters oidcAuthEndpoint=\"$OIDC_AUTH_ENDPOINT\" \\");
    println!("  --parameters oidcBrowserAuthEndpoint=\"$OIDC_BROWSER_AUTH_ENDPOINT\" \\");
    println!("  --parameters oidcTokenEndpoint=\"$OIDC_TOKEN_ENDPOINT\" \\");
    println!("  --parameters oidcUserInfoEndpoint=\"$OIDC_USERINFO_ENDPOINT\" \\");
    println!("  --parameters oidcClientId=\"$OIDC_CLIENT_ID\" \\");
    println!("  --parameters oidcClientSecret=\"$OIDC_CLIENT_SECRET\" \\");
    let application_parameters = format!(
        "@{}",
        prepared
            .private
            .join("application.parameters.json")
            .display()
    );
    println!(
        "  --parameters {} \\",
        shell_quote(OsStr::new(&application_parameters))
    );
    println!("  --parameters nativeAgentNamespace={} \\", args.namespace);
    println!("  --parameters nativeAgentEnvironment=\"$AGENT_ENVIRONMENT\" \\");
    println!("  --parameters nativeAgentGatewayUrl=\"$AGENT_GATEWAY_URL\" \\");
    println!("  --parameters nativeAgentModel=\"$NATIVE_MODEL\" \\");
    println!("  --parameters promptAgentImage=\"$PROMPT_AGENT_IMAGE\" \\");
    println!("  --parameters editorAgentImage=\"$EDITOR_IMAGE\" \\");
    println!("  --parameters nativeUiImage=\"$NATIVE_UI_IMAGE\"");
    Ok(())
}

fn native_browser_ports(trading_port: Option<u16>, keycloak_port: u16) -> Result<[(u16, u16); 2]> {
    let trading_port = trading_port.unwrap_or(3443);
    if trading_port == keycloak_port {
        bail!("Trading and Keycloak browser ports must be different");
    }
    Ok([(trading_port, 3443), (keycloak_port, 8444)])
}

fn native_ui_gateway_service(namespace: &str) -> Result<String> {
    let selector = format!(
        "gateway.envoyproxy.io/owning-gateway-name=ui-ingress,gateway.envoyproxy.io/owning-gateway-namespace={namespace}"
    );
    let output = Command::new("kubectl")
        .args([
            "-n",
            "envoy-gateway-system",
            "get",
            "services",
            "-l",
            &selector,
            "-o",
            "jsonpath={.items[*].metadata.name}",
        ])
        .output()
        .context("failed to find the native UI gateway Service")?;
    if !output.status.success() {
        bail!(
            "failed to find the native UI gateway Service: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    single_service_name(&String::from_utf8(output.stdout)?)
}

fn single_service_name(services: &str) -> Result<String> {
    let mut names = services.split_whitespace();
    let service = names
        .next()
        .context("native UI gateway Service was not found")?;
    if names.next().is_some() {
        bail!("native UI gateway Service ownership is ambiguous");
    }
    Ok(service.to_owned())
}

/// Idempotently provision a local k3d cluster. If a cluster with the
/// requested name already exists we just switch the kubectl context to
/// it; otherwise we create it. Requires `k3d` and `kubectl` on PATH.
fn provision_k3d_cluster(name: &str, dry_run: bool, native: bool) -> Result<()> {
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
        if native {
            cmd.args([
                "cluster",
                "create",
                name,
                "--image",
                "rancher/k3s:v1.35.5-k3s1",
            ]);
        } else {
            cmd.args(["cluster", "create", name]);
        }
        ui::tool_banner("k3d", &render_command(&cmd));
        let status = cmd
            .status()
            .context("failed to spawn `k3d cluster create`")?;
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
    let registry = args.chart_registry.trim_end_matches('/');
    let chart_ref = format!("{registry}/adaptive-apps");
    Ok(Resolution {
        chart_ref: OsString::from(chart_ref),
        version: Some(args.version.clone()),
        // OCI install: profiles are not auto-applied (the profile file
        // is bundled inside the OCI artifact and not extractable until
        // after `helm pull`). Operators using OCI install must pass
        // `--values` for the right profile, or use `--chart-root` to
        // install from source for automatic profile selection.
        values_files: Vec::new(),
    })
}

/// Resolve against the `adaptive-apps` chart at
/// `<chart-root>/adaptive-apps`. Per-portfolio feature flags come from
/// `<chart>/profiles/<portfolio>.yaml`.
fn resolve_local(chart_root: &Path, args: &BootstrapArgs) -> Result<Resolution> {
    let chart = chart_root.join("adaptive-apps");
    if !chart.join("Chart.yaml").is_file() {
        bail!(
            "no chart at {} (Chart.yaml missing).\n\
             --chart-root expects `<chart-root>/adaptive-apps/Chart.yaml`.",
            chart.display()
        );
    }

    let profile_name = args.portfolio.as_str(); // e.g. `core` or `core-ai`
    let profile_file = chart.join("profiles").join(format!("{profile_name}.yaml"));
    require_file(&profile_file, "portfolio profile")?;

    Ok(Resolution {
        chart_ref: chart.into_os_string(),
        version: None,
        values_files: vec![profile_file],
    })
}

fn require_file(path: &Path, what: &str) -> Result<()> {
    if !path.is_file() {
        return Err(anyhow!("missing {what}: {}", path.display()));
    }
    Ok(())
}

fn build_local_chart_dependencies(chart: &Path, dry_run: bool) -> Result<()> {
    let mut command = Command::new("helm");
    command
        .arg("dependency")
        .arg("build")
        .arg(chart)
        .arg("--skip-refresh");
    ui::command(&render_command(&command));
    if dry_run {
        ui::dry_run("not building local chart dependencies");
        return Ok(());
    }
    let status = command
        .status()
        .with_context(|| "failed to spawn helm dependency build")?;
    if !status.success() {
        bail!("helm dependency build exited with status {status}");
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
        Platform::K3s | Platform::K8s | Platform::Arc | Platform::AzureLocal => {
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
    // --platform aks needs enough context to actually talk to the cluster:
    // a subscription to set, plus the rg/cluster pair used to discover the
    // Istio add-on revision and (with --with-radius) to federate identity.
    let mut missing = Vec::new();
    if args.azure_subscription.is_none() {
        missing.push("--azure-subscription");
    }
    if args.resource_group.is_none() {
        missing.push("--resource-group");
    }
    if args.aks_cluster.is_none() {
        missing.push("--aks-cluster");
    }
    if !missing.is_empty() {
        bail!(
            "--platform aks requires {}. Re-run with the missing flag(s) so `ada` can \
             select the subscription and discover the Istio add-on revision.",
            missing.join(", ")
        );
    }

    let sub = args.azure_subscription.as_deref().expect("checked above");
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

    let rg = args.resource_group.as_deref().expect("checked above");
    let cluster = args.aks_cluster.as_deref().expect("checked above");
    let istio_revision = Some(discover_aks_istio_revision(rg, cluster, args.dry_run)?);

    // AKS owns the Istio control plane via the Istio add-on. The chart
    // must not install its own Istio, and the post-install PeerAuthn
    // hook must target the AKS-managed namespace.
    let helm_sets = vec![
        "features.istio.install=false".to_string(),
        format!("istio.namespace={}", args.aks_istio_namespace),
    ];
    Ok(PlatformOutcome {
        helm_sets,
        istio_revision,
    })
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
    if args.with.contains(&Capability::Agents) {
        rad_install.arg("--skip-contour-install");
    }
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

    // Set the workspace's default group so subsequent `rad resource`
    // commands (e.g. `rad resource expose`) work without `--group`.
    let mut rad_group_switch = Command::new("rad");
    rad_group_switch.args(["group", "switch", &args.radius_group]);

    let steps = [
        &rad_install,
        &rad_workspace,
        &rad_switch,
        &rad_group,
        &rad_group_switch,
    ];
    println!();
    ui::heading("Radius bootstrap plan:");
    for s in steps {
        ui::command(&render_command(s));
    }

    let azure_creds_planned = wants_azure_credential_automation(args);
    if azure_creds_planned {
        ui::bullet(&format!(
            "+ reuse registered Radius Azure credential, or provision one (Entra app `{}-radius-app`, federate radius-system SAs, grant Owner on `{}`, rad credential register azure wi)",
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

    for mut cmd in [
        rad_install,
        rad_workspace,
        rad_switch,
        rad_group,
        rad_group_switch,
    ] {
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

    install_radius_recipes(args)?;
    Ok(())
}

/// Print the `--group` / `--environment` the operator should pass to
/// `rad deploy app.bicep`. Useful when defaults don't match the
/// environment name baked into the env bicep (e.g. group=`adaptive`,
/// env=`trading`).
fn print_app_deploy_hint(args: &BootstrapArgs) {
    println!();
    ui::note("when you deploy your app, use:");
    println!(
        "  rad deploy app.bicep --group {} --environment {} --parameters ...",
        args.radius_group, args.radius_environment
    );
}

/// Register the project's custom Radius resource types and deploy the
/// environment bicep (which also pins the recipe versions). Mirrors
/// steps 5–6 of the deploy-to-k8s tutorials. Idempotent: `rad
/// resource-type create` is a re-runnable upsert and `rad deploy`
/// reconciles the environment in place.
fn install_radius_recipes(args: &BootstrapArgs) -> Result<()> {
    if args.skip_radius_recipes {
        return Ok(());
    }
    let types_file = args.radius_dir.join("resource-types/types.yaml");
    let env_bicep = match args.platform {
        Some(Platform::Aks) => args.radius_dir.join("aks-env.bicep"),
        _ => args.radius_dir.join("local-env.bicep"),
    };

    if !args.dry_run {
        if !types_file.exists() {
            ui::note(&format!(
                "skipping Radius recipe install: `{}` not found (pass --radius-dir or --skip-radius-recipes)",
                types_file.display()
            ));
            return Ok(());
        }
        if !env_bicep.exists() {
            ui::note(&format!(
                "skipping Radius env deploy: `{}` not found (pass --radius-dir or --skip-radius-recipes)",
                env_bicep.display()
            ));
            return Ok(());
        }
    }

    println!();
    ui::heading("Radius resource types + environment");
    ui::detail("types file", &types_file.display().to_string());
    ui::detail("env bicep", &env_bicep.display().to_string());
    ui::detail("group", &args.radius_group);
    ui::detail("environment", &args.radius_environment);

    let mut type_cmd = Command::new("rad");
    type_cmd
        .args(["resource-type", "create", "--from-file"])
        .arg(&types_file);

    let mut env_create_cmd = Command::new("rad");
    env_create_cmd.args([
        "environment",
        "create",
        &args.radius_environment,
        "--group",
        &args.radius_group,
    ]);

    let mut deploy_cmd = Command::new("rad");
    deploy_cmd
        .arg("deploy")
        .arg(&env_bicep)
        .args(["--group", &args.radius_group]);
    if matches!(args.platform, Some(Platform::Aks)) {
        if let Some(sub) = &args.azure_subscription {
            deploy_cmd.args(["--parameters", &format!("azureSubscriptionId={sub}")]);
        }
        if let Some(rg) = &args.resource_group {
            deploy_cmd.args(["--parameters", &format!("azureResourceGroup={rg}")]);
        }
    } else if args.with.contains(&Capability::Agents) {
        deploy_cmd.args([
            "--parameters",
            &format!(
                "@{}",
                args.state_dir
                    .join("private/trading.parameters.json")
                    .display()
            ),
        ]);
    }

    ui::command(&render_command(&type_cmd));
    ui::command(&render_command(&env_create_cmd));
    ui::command(&render_command(&deploy_cmd));

    if args.dry_run {
        ui::dry_run("not executing Radius recipe install");
        return Ok(());
    }

    // `rad resource-type create` and `rad environment create` are idempotent
    // upserts; `rad deploy` reconciles in place.
    for mut cmd in [type_cmd, env_create_cmd, deploy_cmd] {
        let label = render_command(&cmd);
        ui::tool_banner("rad", &label);
        let status = cmd
            .status()
            .with_context(|| format!("failed to spawn: {label}"))?;
        if !status.success() {
            bail!("command failed ({status}): {label}");
        }
    }
    ui::ok("Radius recipes ready");
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

fn install_native_gateways(namespace: &str) -> Result<()> {
    if !Command::new("kubectl")
        .args(["get", "namespace", namespace])
        .status()?
        .success()
        && !Command::new("kubectl")
            .args(["create", "namespace", namespace])
            .status()?
            .success()
    {
        bail!("Failed to prepare native namespace {namespace}");
    }
    let status = Command::new("helm")
        .args([
            "upgrade",
            "--install",
            "adaptive-envoy",
            "oci://docker.io/envoyproxy/gateway-helm",
            "--version",
            "v1.9.2",
            "--namespace",
            "envoy-gateway-system",
            "--create-namespace",
            "--set",
            "config.envoyGateway.extensionApis.enableLua=true",
            "--wait",
            "--timeout",
            "10m",
        ])
        .status()?;
    if !status.success() {
        bail!("Native Envoy prerequisite failed");
    }

    for (release, chart) in [
        (
            "agentgateway-crds",
            "oci://cr.agentgateway.dev/charts/agentgateway-crds",
        ),
        (
            "agentgateway",
            "oci://cr.agentgateway.dev/charts/agentgateway",
        ),
    ] {
        let mut command = Command::new("helm");
        command.args([
            "upgrade",
            "--install",
            release,
            chart,
            "--version",
            "v1.6.0",
            "--namespace",
            "agentgateway-system",
            "--create-namespace",
        ]);
        if release == "agentgateway" {
            command
                .arg("--set")
                .arg(format!("rbac.gatewayNamespaces[0]={namespace}"));
        }
        command.args(["--wait", "--timeout", "10m"]);
        if !command.status()?.success() {
            bail!("{release} installation failed");
        }
    }
    Ok(())
}

fn native_prerequisites(args: &BootstrapArgs, prepared: &agents::Prepared) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;
    let namespace = serde_json::json!({
        "apiVersion":"v1","kind":"Namespace","metadata":{"name":args.namespace}
    });
    let mut apply = Command::new("kubectl")
        .args(["apply", "-f", "-"])
        .stdin(Stdio::piped())
        .spawn()?;
    apply
        .stdin
        .take()
        .context("Missing kubectl input pipe")?
        .write_all(&serde_json::to_vec(&namespace)?)?;
    if !apply.wait()?.success() {
        bail!("Native namespace preparation failed");
    }
    let mut commands = vec![
        {
            let mut command = Command::new("kubectl");
            command
                .args(["apply", "-f"])
                .arg(prepared.private.join("secrets.json"));
            command
        },
        {
            let mut command = Command::new("kubectl");
            command
                .args(["apply", "-f"])
                .arg(args.radius_dir.join("prerequisites.yaml"));
            command
        },
        {
            let mut command = Command::new("rad");
            command.args([
                "environment",
                "create",
                "agents",
                "--group",
                &args.radius_group,
                "--namespace",
                &args.namespace,
            ]);
            command
        },
        {
            let mut command = Command::new("rad");
            command
                .args(["bicep", "publish-extension", "--from-file"])
                .arg(args.radius_dir.join("resource-types/types.yaml"))
                .arg("--target")
                .arg(args.radius_dir.join("types.tgz"));
            command
        },
    ];
    for command in &mut commands {
        if !command.status()?.success() {
            bail!("Native prerequisite failed: {}", render_command(command));
        }
    }
    Ok(())
}

fn maybe_provision_oidc_client(args: &BootstrapArgs) -> Result<()> {
    if args.skip_oidc_client || args.with.contains(&Capability::Agents) {
        return Ok(());
    }
    let service = format!("{}-keycloak", args.release);
    if !args.dry_run && !keycloak_service_present(&args.namespace, &service)? {
        ui::note(&format!(
            "keycloak service `{}/{service}` not found; skipping OIDC client provisioning",
            args.namespace
        ));
        return Ok(());
    }
    // Derive web-origins from redirect URIs by stripping trailing `/*`;
    // always include the canonical frontend origin too.
    let mut web_origins: Vec<String> = args
        .oidc_redirect_uris
        .iter()
        .filter_map(|u| u.strip_suffix("/*").map(str::to_string))
        .collect();
    if !web_origins.iter().any(|o| o == "http://localhost:3000") {
        web_origins.push("http://localhost:3000".to_string());
    }
    let setup = SetupClientArgs {
        namespace: args.namespace.clone(),
        release: args.release.clone(),
        client_id: args.oidc_client_id.clone(),
        redirect_uris: args.oidc_redirect_uris.clone(),
        web_origins,
        realm: args.oidc_realm.clone(),
        admin_user: args.keycloak_admin_user.clone(),
        admin_password: args.keycloak_admin_password.clone(),
        admin_password_file: None,
        client_secret_file: None,
        local_port: args.keycloak_port.unwrap_or(8080),
        pod_ready_timeout_secs: 180,
        keep_port_forward: args.keep_port_forward,
        dry_run: args.dry_run,
    };
    let (result, pf) = oidc::provision_client_keep(&setup)?;
    oidc::print_exports(&setup, &result);
    if let Some(mut pf) = pf {
        println!();
        ui::note(&format!(
            "keeping port-forward to svc/{}-keycloak on http://localhost:{} — press Ctrl-C to stop",
            args.release, setup.local_port
        ));
        ui::note("run `rad deploy app.bicep ...` in another terminal while this stays open");
        let _ = pf.wait();
    }
    Ok(())
}

/// Probe for `svc/<name>` in `namespace`. Returns Ok(false) on a clean
/// NotFound, Err on any other kubectl failure.
fn keycloak_service_present(namespace: &str, service: &str) -> Result<bool> {
    let out = Command::new("kubectl")
        .args(["-n", namespace, "get", "svc", service, "-o", "name"])
        .output()
        .context("failed to run `kubectl get svc`")?;
    if out.status.success() {
        return Ok(true);
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("NotFound") || stderr.contains("not found") {
        return Ok(false);
    }
    bail!(
        "`kubectl get svc -n {namespace} {service}` failed: {}",
        stderr.trim()
    );
}

#[cfg(test)]
mod tests {
    use super::{native_browser_ports, parse_radius_azure_credential, single_service_name};

    #[test]
    fn native_browser_port_overrides_keep_gateway_listener_ports() {
        assert_eq!(
            native_browser_ports(None, 8444).unwrap(),
            [(3443, 3443), (8444, 8444)]
        );
        assert_eq!(
            native_browser_ports(Some(3445), 8445).unwrap(),
            [(3445, 3443), (8445, 8444)]
        );
        assert!(native_browser_ports(Some(8445), 8445).is_err());
    }

    #[test]
    fn browser_port_flags_validate_ranges_and_preserve_defaults() {
        #[derive(clap::Parser)]
        struct TestCli {
            #[command(flatten)]
            args: super::BootstrapArgs,
        }
        let parse = |extra: &[&str]| {
            <TestCli as clap::Parser>::try_parse_from(
                ["ada", "--portfolio", "min-ai", "--with", "agents"]
                    .into_iter()
                    .chain(extra.iter().copied()),
            )
        };
        let defaults = parse(&[]).unwrap().args;
        assert_eq!(defaults.keycloak_port, None);
        assert_eq!(defaults.trading_port, None);
        let custom = parse(&["--keycloak-port", "8445", "--trading-port", "3445"])
            .unwrap()
            .args;
        assert_eq!(custom.keycloak_port, Some(8445));
        assert_eq!(custom.trading_port, Some(3445));
        for flag in ["--keycloak-port", "--trading-port"] {
            assert!(parse(&[flag, "0"]).is_err());
            assert!(parse(&[flag, "65536"]).is_err());
        }
    }

    #[test]
    fn native_gateway_service_must_have_one_owner() {
        assert_eq!(
            single_service_name("envoy-ui-123\n").unwrap(),
            "envoy-ui-123"
        );
        assert!(single_service_name("").is_err());
        assert!(single_service_name("envoy-ui-1 envoy-ui-2").is_err());
    }

    #[test]
    fn registered_radius_azure_credential_is_reused() {
        assert!(parse_radius_azure_credential(
            br#"[{"Name":"azure","Enabled":true},{"Name":"aws","Enabled":false}]"#
        )
        .unwrap());
    }

    #[test]
    fn unregistered_radius_azure_credential_needs_provisioning() {
        for output in [
            "[]",
            r#"[{"Name":"azure","Enabled":false}]"#,
            r#"[{"Name":"aws","Enabled":true}]"#,
        ] {
            assert!(!parse_radius_azure_credential(output.as_bytes()).unwrap());
        }
    }

    #[test]
    fn radius_057_credential_list_banner_is_supported() {
        let output = concat!(
            "Listing credentials for all cloud providers for Radius installation ",
            "\"Kubernetes (context=ada-ignite)\"...\r\n",
            "[\r\n  {\"Name\":\"azure\",\"Enabled\":true}\r\n]\r\n"
        );
        assert!(parse_radius_azure_credential(output.as_bytes()).unwrap());
    }

    #[test]
    fn invalid_radius_credential_output_is_not_treated_as_absent() {
        for output in [
            "",
            "not JSON",
            "null",
            "{}",
            r#"[{"Name":"azure"}]"#,
            r#"[{"Name":"azure","Enabled":"true"}]"#,
            r#"{"Name":"azure","Enabled":true}"#,
            "unexpected diagnostic\n[]",
            "Listing credentials for all cloud providers for Radius installation \"test\"...\nnot JSON",
        ] {
            assert!(
                parse_radius_azure_credential(output.as_bytes()).is_err(),
                "accepted invalid credential output: {output}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn radius_credential_check_precedes_azure_cli() {
        use std::os::unix::fs::PermissionsExt;

        const CHILD: &str = "ADA_TEST_RADIUS_CREDENTIAL_CASE";
        if let Ok(case) = std::env::var(CHILD) {
            #[derive(clap::Parser)]
            struct TestCli {
                #[command(flatten)]
                args: super::BootstrapArgs,
            }
            let cli = <TestCli as clap::Parser>::parse_from([
                "ada",
                "--portfolio",
                "min",
                "--platform",
                "aks",
                "--with-radius",
                "--radius-workspace",
                "test-workspace",
                "--resource-group",
                "test-rg",
                "--aks-cluster",
                "test-aks",
            ]);
            let result = super::provision_azure_credential(&cli.args);
            match case.as_str() {
                "registered" => result.unwrap(),
                "failed" => assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("inspection denied"),
                    "inspection failure must not fall through to Azure CLI"
                ),
                "invalid" => assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("invalid JSON credential status"),
                    "invalid output must not fall through to Azure CLI"
                ),
                _ => panic!("unexpected test case"),
            }
            return;
        }

        let directory = std::env::temp_dir().join(format!(
            "ada-radius-credential-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let rad = directory.join("rad");
        std::fs::write(
            &rad,
            r#"#!/bin/sh
if [ "$*" != "credential list --workspace test-workspace -o json" ]; then
    echo "unexpected rad arguments" >&2
    exit 2
fi
case "$ADA_TEST_RADIUS_CREDENTIAL_CASE" in
    registered)
        printf '%s\n' 'Listing credentials for all cloud providers for Radius installation "Kubernetes (context=test)"...'
        printf '%s\n' '[{"Name":"azure","Enabled":true}]'
        ;;
    failed) echo "inspection denied" >&2; exit 1 ;;
    invalid) printf '%s\n' 'not JSON' ;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&rad, std::fs::Permissions::from_mode(0o700)).unwrap();

        let results: Vec<_> = ["registered", "failed", "invalid"]
            .into_iter()
            .map(|case| {
                let output = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "commands::bootstrap::tests::radius_credential_check_precedes_azure_cli",
                        "--nocapture",
                    ])
                    .env(CHILD, case)
                    // No az executable: even tool discovery must be skipped on reuse.
                    .env("PATH", &directory)
                    .output()
                    .unwrap();
                (case, output)
            })
            .collect();
        std::fs::remove_file(rad).unwrap();
        std::fs::remove_dir(directory).unwrap();
        for (case, output) in results {
            assert!(
                output.status.success(),
                "{case}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

fn radius_azure_credential_registered(workspace: &str) -> Result<bool> {
    let output = Command::new("rad")
        .args(["credential", "list", "--workspace", workspace, "-o", "json"])
        .output()
        .context("failed to run `rad credential list`")?;
    if !output.status.success() {
        bail!(
            "`rad credential list --workspace {workspace} -o json` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    parse_radius_azure_credential(&output.stdout)
}

fn parse_radius_azure_credential(output: &[u8]) -> Result<bool> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct CredentialStatus {
        name: String,
        enabled: bool,
    }

    let output = std::str::from_utf8(output)
        .context("invalid UTF-8 credential status from `rad credential list`")?
        .trim();
    // Radius 0.57 writes this informational line to stdout even with JSON output.
    let json = match output.split_once('\n') {
        Some((banner, json))
            if banner.starts_with(
                "Listing credentials for all cloud providers for Radius installation \"",
            ) && banner.trim_end().ends_with("\"...") =>
        {
            json
        }
        _ => output,
    };
    let credentials: Vec<CredentialStatus> = serde_json::from_str(json)
        .context("invalid JSON credential status from `rad credential list`")?;
    Ok(credentials
        .iter()
        .any(|credential| credential.name == "azure" && credential.enabled))
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
/// Reuse a registered Radius credential without accessing Microsoft Graph.
/// Otherwise provision the Entra app, federation, and role assignment.
fn provision_azure_credential(args: &BootstrapArgs) -> Result<()> {
    ensure_tool("rad")?;

    if !args.dry_run && radius_azure_credential_registered(&args.radius_workspace)? {
        ui::note(&format!(
            "Azure credential already registered with Radius workspace `{}`; \
             skipping Entra application, federation, RBAC, and credential provisioning \
             (existing access is not revalidated)",
            args.radius_workspace
        ));
        return Ok(());
    }

    ensure_tool("az")?;

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
    ui::detail(
        "rbac scope",
        &format!("/subscriptions/<sub>/resourceGroups/{rg} (Owner)"),
    );

    if args.dry_run {
        ui::dry_run("would check the Radius Azure credential; only if absent, discover AKS OIDC issuer, create/reuse Entra app, federate SAs, grant Owner, rad credential register azure wi");
        return Ok(());
    }

    let oidc_issuer = az_query(
        &[
            "aks",
            "show",
            "-g",
            rg,
            "-n",
            cluster,
            "--query",
            "oidcIssuerProfile.issuerUrl",
            "-o",
            "tsv",
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
            "ad",
            "app",
            "list",
            "--display-name",
            &app_name,
            "--query",
            "[0].appId",
            "-o",
            "tsv",
        ],
        "existing Entra app id",
    )?;
    let app_id = if existing.is_empty() {
        ui::step(&format!("creating Entra application `{app_name}`"));
        az_query(
            &[
                "ad",
                "app",
                "create",
                "--display-name",
                &app_name,
                "--query",
                "appId",
                "-o",
                "tsv",
            ],
            "new Entra app id",
        )?
    } else {
        ui::step(&format!(
            "reusing existing Entra application `{app_name}` ({existing})"
        ));
        existing
    };
    let app_object_id = az_query(
        &[
            "ad", "app", "show", "--id", &app_id, "--query", "id", "-o", "tsv",
        ],
        "Entra app object id",
    )?;

    // Federate the four Radius service accounts. `az ad app
    // federated-credential create` is not idempotent, so list first.
    let existing_fc = az_query(
        &[
            "ad",
            "app",
            "federated-credential",
            "list",
            "--id",
            &app_object_id,
            "--query",
            "[].name",
            "-o",
            "tsv",
        ],
        "existing federated credentials",
    )?;
    let existing_fc: std::collections::HashSet<&str> = existing_fc
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    for (fc_name, sa) in RADIUS_FEDERATED_SAS {
        if existing_fc.contains(fc_name) {
            ui::step(&format!(
                "federated credential `{fc_name}` already exists, skipping"
            ));
            continue;
        }
        let subject = format!("system:serviceaccount:radius-system:{sa}");
        let params = format!(
            r#"{{"name":"{fc_name}","issuer":"{oidc_issuer}","subject":"{subject}","description":"Radius {sa} service account","audiences":["api://AzureADTokenExchange"]}}"#
        );
        ui::step(&format!(
            "creating federated credential `{fc_name}` for {subject}"
        ));
        let status = Command::new("az")
            .args([
                "ad",
                "app",
                "federated-credential",
                "create",
                "--id",
                &app_object_id,
                "--parameters",
                &params,
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
            "role",
            "assignment",
            "create",
            "--assignee",
            &app_id,
            "--role",
            "Owner",
            "--scope",
            &scope,
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
            "credential",
            "register",
            "azure",
            "wi",
            "--client-id",
            &app_id,
            "--tenant-id",
            &tenant_id,
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
