//! `ada package` — analyze a source folder and emit a Radius app
//! definition (`app.bicep`) using a selectable generation **strategy**.
//!
//! Strategies (`--strategy`):
//!
//! * **compose** (default): parse `docker-compose.yml` from the input
//!   folder and map services to Radius resources using a small ruleset
//!   (postgres → `postgreSqlDatabases`, mosquitto/MQTT → `mqttBrokers`,
//!   observability infra → skipped, everything else →
//!   `Applications.Core/containers`).
//! * **llm**: collect the docker-compose file plus a short bundle of
//!   source signals (Dockerfiles, READMEs, manifests) and ask an
//!   OpenAI-compatible chat completion endpoint to emit a bicep file. The
//!   compose result is included in the prompt as a starting point.
//! * **skill**: same as `llm`, but the model is driven by an external
//!   agent-skill pack (e.g. `radius-project/radius-skills` `app-modeling`)
//!   fetched via `ada skill add`. The skill's rules become the system
//!   prompt.
//!
//! All LLM-backed strategies can run inside a **generate → critique →
//! refine** loop (`--critic`): each candidate is checked by a hybrid set
//! of critics — a deterministic `bicep build` compile and an LLM judge
//! scored against the skill's validation checklist — and the issues are
//! fed back to the generator until the candidate passes or `--max-iters`
//! is reached.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, ValueEnum};
use serde::Deserialize;

use crate::commands::catalog::{self, Catalog};
use crate::commands::skill;
use crate::home;
use crate::ui;

const MAX_LLM_CONTEXT_BYTES: usize = 32 * 1024;
const DEFAULT_LLM_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_LLM_MODEL: &str = "gpt-4o-mini";

/// Generation strategy for `ada package`. New strategies slot in by
/// adding a variant here and a matching [`Generator`] implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Strategy {
    /// Deterministic docker-compose analyzer (no network, no API key).
    Compose,
    /// Single-shot OpenAI-compatible LLM call seeded with the compose plan.
    Llm,
    /// LLM driven by an external agent-skill pack (`ada skill add`).
    Skill,
}

#[derive(Debug, Args)]
pub struct PackageArgs {
    /// Input folder to analyze (e.g. `src/`). Must contain a
    /// `docker-compose.yml`.
    #[arg(long, short = 'i')]
    input: PathBuf,

    /// Output file to write (e.g. `app.bicep`). Existing files are
    /// overwritten unless `--dry-run` is set.
    #[arg(long, short = 'o')]
    output: PathBuf,

    /// Logical application name. Used for the
    /// `Applications.Core/applications` resource name. Defaults to the
    /// input directory name.
    #[arg(long)]
    app_name: Option<String>,

    /// Print the generated bicep to stdout without writing the file.
    #[arg(long)]
    dry_run: bool,

    /// Generation strategy to use.
    #[arg(long, value_enum, default_value_t = Strategy::Compose)]
    strategy: Strategy,

    /// Deprecated alias for `--strategy llm`. Kept for backward
    /// compatibility.
    #[arg(long, hide = true)]
    llm: bool,

    /// Skill repository (`owner/repo`) for `--strategy skill`. Must be
    /// cached via `ada skill add`.
    #[arg(long, default_value = skill::DEFAULT_SKILL_REPO)]
    skill: String,

    /// Skill name within the repo for `--strategy skill`.
    #[arg(long, default_value = skill::DEFAULT_SKILL_NAME)]
    skill_name: String,

    /// Path to a Radius resource-type catalog (`types.yaml`). When omitted,
    /// the repo-local `radius/resource-types/types.yaml` or the installed
    /// `$ADA_HOME/radius/resource-types/types.yaml` is used if present.
    #[arg(long)]
    types: Option<PathBuf>,

    /// Path to an existing "truth" `app.bicep` to use as the baseline for
    /// update/iteration scenarios. The reference grounds generation
    /// (preserve its structure/naming/wiring) and adds a critic that flags
    /// regressions against it. Pair with `--explain` to describe the diffs.
    #[arg(long)]
    truth_ref: Option<PathBuf>,

    /// After generating, explain how the result differs from the
    /// `--truth-ref` baseline and why. Requires `--truth-ref` and an API
    /// key. Useful when iterating on an existing definition.
    #[arg(long, requires = "truth_ref")]
    explain: bool,

    /// Run the generate → critique → refine loop. Only meaningful for the
    /// `llm` and `skill` strategies.
    #[arg(long)]
    critic: bool,

    /// Maximum number of generate/refine iterations when `--critic` is set.
    #[arg(long, default_value_t = 2)]
    max_iters: u32,

    /// Chat-completion endpoint URL (OpenAI-compatible).
    #[arg(long, default_value = DEFAULT_LLM_ENDPOINT, env = "ADA_LLM_ENDPOINT")]
    llm_endpoint: String,

    /// Model name for the LLM call.
    #[arg(long, default_value = DEFAULT_LLM_MODEL, env = "ADA_LLM_MODEL")]
    llm_model: String,

    /// API key for the LLM endpoint. Falls back to `OPENAI_API_KEY`.
    #[arg(long, env = "OPENAI_API_KEY", hide_env_values = true)]
    llm_api_key: Option<String>,
}

pub fn run(args: PackageArgs) -> Result<()> {
    if !args.input.is_dir() {
        bail!("input folder does not exist or is not a directory: {}", args.input.display());
    }
    let app_name = args.app_name.clone().unwrap_or_else(|| {
        args.input
            .canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "app".to_string())
    });

    // `--llm` is a backward-compatible alias for `--strategy llm`.
    let strategy = if args.llm && args.strategy == Strategy::Compose {
        Strategy::Llm
    } else {
        args.strategy
    };

    ui::heading("Package");
    ui::detail("input", &args.input.display().to_string());
    ui::detail("output", &args.output.display().to_string());
    ui::detail("app name", &app_name);
    ui::detail("strategy", strategy_label(strategy));
    if args.critic && strategy != Strategy::Compose {
        ui::detail("critic", &format!("on (max {} iters)", args.max_iters));
    }

    let compose_path = find_compose(&args.input);
    match &compose_path {
        Some(p) => ui::detail("compose", &p.display().to_string()),
        None => {
            // The deterministic analyzer has nothing to work from without a
            // compose file, so the `compose` strategy requires one. The
            // LLM/skill strategies fall back to the raw source bundle.
            if strategy == Strategy::Compose {
                bail!(
                    "no docker-compose file found under {} (looked for docker-compose.yml/yaml, \
                     compose.yml/yaml). The `compose` strategy requires one; try `--strategy llm` \
                     or `--strategy skill` to model from source instead.",
                    args.input.display()
                );
            }
            ui::detail("compose", "(none — modeling from source)");
        }
    }

    // Load the repo's resource-type catalog so generation uses the types
    // this platform actually defines (not guessed upstream ones).
    let catalog = Catalog::load(args.types.as_deref(), &args.input)?;
    match &catalog {
        Some(c) => ui::detail(
            "types",
            &format!("{} ({} types from {})", c.namespace, c.types.len(), c.source.display()),
        ),
        None => ui::note(
            "no resource-type catalog found; generation will not be grounded in platform types (pass --types or run `ada init`)",
        ),
    }

    // Load the optional "truth" reference used to ground generation and to
    // critique/explain divergences in update scenarios.
    let truth_ref = match &args.truth_ref {
        Some(p) => {
            let content = fs::read_to_string(p)
                .with_context(|| format!("failed to read truth reference {}", p.display()))?;
            ui::detail("truth ref", &p.display().to_string());
            if strategy == Strategy::Compose {
                ui::note(
                    "the compose strategy ignores --truth-ref for generation; it is still used by --explain",
                );
            }
            Some(content)
        }
        None => None,
    };

    // Build the analyzer plan from the compose file when present; otherwise
    // start from an empty plan (LLM/skill strategies use the source bundle).
    let plan = match &compose_path {
        Some(p) => {
            ui::step("analyzing docker-compose services");
            let plan = analyze_compose(p, &app_name)?;
            summarize_plan(&plan);
            plan
        }
        None => empty_plan(&app_name),
    };
    let static_bicep = render_bicep(&plan, catalog.as_ref());

    // The source bundle is only needed for LLM-backed strategies.
    let context_bundle = if strategy == Strategy::Compose {
        String::new()
    } else {
        collect_context_bundle(&args.input, compose_path.as_deref())?
    };

    let ctx = PackageContext {
        plan,
        static_bicep,
        context_bundle,
        catalog,
        truth_ref,
    };

    // Build the selected generator.
    let llm_client = || -> Result<LlmClient> {
        let api_key = args
            .llm_api_key
            .clone()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                anyhow!("this strategy requires an API key; set OPENAI_API_KEY or pass --llm-api-key")
            })?;
        Ok(LlmClient {
            endpoint: args.llm_endpoint.clone(),
            model: args.llm_model.clone(),
            api_key,
        })
    };

    let generator: Box<dyn Generator> = match strategy {
        Strategy::Compose => Box::new(ComposeGenerator),
        Strategy::Llm => Box::new(LlmGenerator { client: llm_client()? }),
        Strategy::Skill => {
            ui::step(&format!("loading skill {}/{}", args.skill, args.skill_name));
            let pack = skill::load_pack(&args.skill, &args.skill_name)?;
            if !pack.sha.is_empty() {
                ui::detail("skill sha", &pack.sha.chars().take(7).collect::<String>());
            }
            Box::new(SkillGenerator { client: llm_client()?, pack })
        }
    };

    let final_bicep = if args.critic && strategy != Strategy::Compose {
        // Assemble the critic set: catalog grounding + deterministic
        // compile + LLM judge.
        let mut critics: Vec<Box<dyn Critic>> = Vec::new();
        if ctx.catalog.is_some() {
            ui::detail("catalog critic", "Radius.Resources type/version check");
            critics.push(Box::new(CatalogCritic));
        }
        match BicepBuildCritic::discover() {
            Some(c) => {
                ui::detail("compile critic", c.label());
                critics.push(Box::new(c));
            }
            None => ui::note("no bicep/rad/az found on PATH; skipping compile critic"),
        }
        let rubric = match generator.skill_instructions() {
            Some(p) => p.to_string(),
            None => BUILTIN_RUBRIC.to_string(),
        };
        critics.push(Box::new(ChecklistCritic { client: llm_client()?, rubric }));
        if ctx.truth_ref.is_some() {
            ui::detail("truth critic", "regression check vs --truth-ref");
            critics.push(Box::new(TruthRefCritic { client: llm_client()? }));
        }
        refine(generator.as_ref(), &critics, &ctx, args.max_iters)?
    } else {
        if args.critic {
            ui::note("--critic has no effect for the compose strategy; emitting analyzer output");
        }
        ui::step(&format!("generating ({})", strategy_label(strategy)));
        let out = generator.generate(&ctx, None)?;
        ui::ok("candidate generated");
        out
    };

    // Final safeguard: strip any stray prose/markdown the model may have
    // wrapped around the template, regardless of which strategy produced it.
    let final_bicep = extract_bicep(&final_bicep);

    // Explain how the result diverges from the baseline, when requested.
    if args.explain {
        match ctx.truth_ref.as_deref() {
            Some(reference) => {
                ui::step("explaining divergence from truth reference");
                let explanation = explain_divergence(&llm_client()?, reference, &final_bicep)?;
                ui::heading("Divergence from --truth-ref");
                println!("{}", explanation.trim());
                println!();
            }
            // Unreachable: clap enforces `--explain` requires `--truth-ref`.
            None => ui::note("--explain requires --truth-ref; skipping explanation"),
        }
    }

    if args.dry_run {
        ui::dry_run(&format!("not writing {} (printed below)", args.output.display()));
        println!();
        println!("{final_bicep}");
        return Ok(());
    }

    if let Some(parent) = args.output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory {}", parent.display())
            })?;
        }
    }
    fs::write(&args.output, &final_bicep)
        .with_context(|| format!("failed to write {}", args.output.display()))?;
    ui::ok(&format!("wrote {}", args.output.display()));
    Ok(())
}

fn strategy_label(s: Strategy) -> &'static str {
    match s {
        Strategy::Compose => "compose (static)",
        Strategy::Llm => "llm-assisted",
        Strategy::Skill => "skill-driven",
    }
}


// ---------------------------------------------------------------------------
// Plan extraction
// ---------------------------------------------------------------------------

/// What we ultimately render to bicep. Captured as a small struct so
/// rendering and the LLM prompt can both consume the same shape.
#[derive(Debug, Clone)]
struct Plan {
    app_name: String,
    /// Custom resources detected: `(resource_name, kind)`.
    resources: Vec<DetectedResource>,
    /// Container services we actually render.
    containers: Vec<ContainerSpec>,
    /// Services skipped because they map to platform infra (otel, prom, …).
    skipped: Vec<String>,
}

#[derive(Debug, Clone)]
struct DetectedResource {
    /// Bicep symbolic name (e.g. `tradingDb`).
    symbol: String,
    /// Resource name (e.g. `app-db`).
    name: String,
    kind: ResourceKind,
    /// docker-compose service that produced this resource — used to wire
    /// container `connections`.
    source_service: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceKind {
    Postgres,
    Mqtt,
}

impl ResourceKind {
    fn type_ref(self) -> &'static str {
        match self {
            Self::Postgres => "Radius.Resources/postgreSqlDatabases@2025-08-01-preview",
            Self::Mqtt => "Radius.Resources/mqttBrokers@2025-08-01-preview",
        }
    }

    /// Short type name as defined in the resource-type catalog
    /// (`radius/resource-types/types.yaml`).
    fn catalog_type_name(self) -> &'static str {
        match self {
            Self::Postgres => "postgreSqlDatabases",
            Self::Mqtt => "mqttBrokers",
        }
    }

    /// Connection key used in `connections: { <key>: { source: ... } }`.
    fn connection_key(self) -> &'static str {
        match self {
            Self::Postgres => "db",
            Self::Mqtt => "mqtt",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Mqtt => "mqtt",
        }
    }
}

#[derive(Debug, Clone)]
struct ContainerSpec {
    /// docker-compose service name (also the Radius container name).
    name: String,
    image: String,
    /// First `containerPort` discovered, if any.
    container_port: Option<u16>,
    /// Static env vars (literal value).
    env: BTreeMap<String, String>,
    /// Symbol names of resources this container depends on (subset of
    /// `Plan.resources`).
    connections: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Compose {
    #[serde(default)]
    services: BTreeMap<String, ComposeService>,
}

#[derive(Debug, Default, Deserialize)]
struct ComposeService {
    #[serde(default)]
    image: Option<String>,
    #[serde(default, rename = "build")]
    _build: Option<serde_yaml::Value>,
    #[serde(default)]
    ports: Vec<serde_yaml::Value>,
    #[serde(default)]
    environment: Option<serde_yaml::Value>,
    #[serde(default)]
    depends_on: Option<serde_yaml::Value>,
}

fn find_compose(dir: &Path) -> Option<PathBuf> {
    for candidate in ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"] {
        let p = dir.join(candidate);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// An empty plan used when no docker-compose file is present. The app name
/// is still rendered (and the LLM/skill strategies work from the source
/// bundle instead of the analyzer plan).
fn empty_plan(app_name: &str) -> Plan {
    Plan {
        app_name: app_name.to_string(),
        resources: Vec::new(),
        containers: Vec::new(),
        skipped: Vec::new(),
    }
}

fn analyze_compose(compose_path: &Path, app_name: &str) -> Result<Plan> {
    let text = fs::read_to_string(compose_path)
        .with_context(|| format!("failed to read {}", compose_path.display()))?;
    let compose: Compose = serde_yaml::from_str(&text)
        .with_context(|| format!("failed to parse {} as YAML", compose_path.display()))?;

    let mut resources: Vec<DetectedResource> = Vec::new();
    let mut container_candidates: Vec<(String, ComposeService)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for (name, svc) in compose.services {
        let image_lower = svc
            .image
            .as_deref()
            .unwrap_or("")
            .to_lowercase();

        if is_observability_image(&image_lower) {
            skipped.push(name);
            continue;
        }
        if let Some(kind) = detect_resource_kind(&image_lower) {
            resources.push(DetectedResource {
                symbol: sanitize_symbol(&format!("{app_name}-{}", kind.label())),
                name: format!("{app_name}-{}", kind.label()),
                kind,
                source_service: name,
            });
            continue;
        }
        container_candidates.push((name, svc));
    }

    let mut containers = Vec::new();
    for (name, svc) in container_candidates {
        let image = pick_image(&name, &svc);
        let env = extract_env(svc.environment.as_ref());
        let container_port = extract_container_port(&svc.ports);
        let depends = extract_depends(svc.depends_on.as_ref());
        let connections = resources
            .iter()
            .filter(|r| depends.contains(&r.source_service))
            .map(|r| r.symbol.clone())
            .collect();
        containers.push(ContainerSpec {
            name,
            image,
            container_port,
            env,
            connections,
        });
    }

    Ok(Plan {
        app_name: app_name.to_string(),
        resources,
        containers,
        skipped,
    })
}

fn is_observability_image(image: &str) -> bool {
    const PATTERNS: &[&str] = &[
        "otel",
        "opentelemetry",
        "prometheus",
        "/prom/",
        "grafana",
        "jaeger",
        "zipkin",
        "loki",
        "tempo",
        "fluent",
        "vector:",
    ];
    PATTERNS.iter().any(|p| image.contains(p))
}

fn detect_resource_kind(image: &str) -> Option<ResourceKind> {
    if image.contains("postgres") || image.contains("postgresql") {
        return Some(ResourceKind::Postgres);
    }
    if image.contains("mosquitto") || image.contains("emqx") || image.contains("hivemq") {
        return Some(ResourceKind::Mqtt);
    }
    None
}

fn pick_image(service_name: &str, svc: &ComposeService) -> String {
    if let Some(img) = &svc.image {
        return img.clone();
    }
    // Build context — caller is expected to override imageRegistry/imageTag.
    format!("${{imageRegistry}}/{service_name}:${{imageTag}}")
}

fn extract_env(env: Option<&serde_yaml::Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(env) = env else { return out };
    match env {
        serde_yaml::Value::Mapping(m) => {
            for (k, v) in m {
                if let Some(key) = k.as_str() {
                    out.insert(key.to_string(), yaml_scalar_to_string(v));
                }
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                if let Some(s) = item.as_str() {
                    if let Some((k, v)) = s.split_once('=') {
                        out.insert(k.to_string(), v.to_string());
                    } else {
                        out.insert(s.to_string(), String::new());
                    }
                }
            }
        }
        _ => {}
    }
    out
}

fn yaml_scalar_to_string(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Null => String::new(),
        other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
    }
}

fn extract_container_port(ports: &[serde_yaml::Value]) -> Option<u16> {
    for p in ports {
        let candidate = match p {
            serde_yaml::Value::String(s) => s.clone(),
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::Mapping(m) => {
                // Long syntax: { target: 8080, published: 5001 }
                m.get(serde_yaml::Value::String("target".into()))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            }
            _ => continue,
        };
        // Short syntax "host:container" or just "container".
        let internal = candidate.rsplit(':').next().unwrap_or(&candidate);
        let internal = internal.split('/').next().unwrap_or(internal); // strip /tcp
        if let Ok(n) = internal.trim().parse::<u16>() {
            return Some(n);
        }
    }
    None
}

fn extract_depends(depends: Option<&serde_yaml::Value>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(depends) = depends else { return out };
    match depends {
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                if let Some(s) = item.as_str() {
                    out.insert(s.to_string());
                }
            }
        }
        serde_yaml::Value::Mapping(m) => {
            for (k, _) in m {
                if let Some(s) = k.as_str() {
                    out.insert(s.to_string());
                }
            }
        }
        _ => {}
    }
    out
}

fn sanitize_symbol(s: &str) -> String {
    // Camel-case, drop non-identifier chars. e.g. "trading-db" → "tradingDb".
    let mut out = String::new();
    let mut upper_next = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if upper_next {
                out.extend(c.to_uppercase());
                upper_next = false;
            } else {
                out.push(c);
            }
        } else {
            upper_next = !out.is_empty();
        }
    }
    if out.is_empty() {
        return "app".into();
    }
    // Ensure lowercase first char.
    let mut chars = out.chars();
    let first = chars.next().unwrap().to_ascii_lowercase();
    let rest: String = chars.collect();
    format!("{first}{rest}")
}

fn summarize_plan(plan: &Plan) {
    ui::bullet(&format!("application: {}", plan.app_name));
    if plan.resources.is_empty() {
        ui::bullet("resources : (none detected)");
    } else {
        ui::bullet("resources :");
        for r in &plan.resources {
            ui::bullet(&format!("  - {} ({})", r.name, r.kind.label()));
        }
    }
    ui::bullet("containers:");
    for c in &plan.containers {
        let port = c.container_port.map(|p| format!(" :{p}")).unwrap_or_default();
        let conns = if c.connections.is_empty() {
            String::new()
        } else {
            format!(" -> {}", c.connections.join(","))
        };
        ui::bullet(&format!("  - {}{}{}", c.name, port, conns));
    }
    if !plan.skipped.is_empty() {
        ui::bullet(&format!("skipped (platform infra): {}", plan.skipped.join(", ")));
    }
}

// ---------------------------------------------------------------------------
// Bicep rendering
// ---------------------------------------------------------------------------

fn render_bicep(plan: &Plan, catalog: Option<&Catalog>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// {}.bicep — Radius application generated by `ada package`.\n//\n// Edit freely; this file is a starting point.\n\nextension radius\nextension radiusResources\n\n@description('The ID of the Radius Environment. Injected automatically by the rad CLI.')\nparam environment string\n\n@description('Container image registry prefix.')\nparam imageRegistry string = ''\n\n@description('Container image tag.')\nparam imageTag string = 'latest'\n\n@description('OTLP collector endpoint, e.g. http://otel-collector.<ns>:4318')\nparam otelCollectorEndpoint string = ''\n\n",
        plan.app_name,
    ));

    let app_symbol = sanitize_symbol(&format!("{}-app", plan.app_name));
    out.push_str(&format!(
        "resource {app_symbol} 'Applications.Core/applications@2023-10-01-preview' = {{\n  name: '{}'\n  properties: {{\n    environment: environment\n  }}\n}}\n\n",
        plan.app_name
    ));

    for r in &plan.resources {
        let extra = match r.kind {
            ResourceKind::Postgres => "    size: 'S'\n",
            ResourceKind::Mqtt => "",
        };
        // Prefer the catalog's fully-qualified `type@apiVersion`; fall back
        // to the built-in constant when no catalog is loaded.
        let type_ref = catalog
            .and_then(|c| c.type_ref(r.kind.catalog_type_name()))
            .unwrap_or_else(|| r.kind.type_ref().to_string());
        out.push_str(&format!(
            "resource {} '{}' = {{\n  name: '{}'\n  properties: {{\n    environment: environment\n    application: {app_symbol}.id\n{extra}  }}\n}}\n\n",
            r.symbol, type_ref, r.name,
        ));
    }

    for c in &plan.containers {
        let image_expr = format!("'{}'", c.image);
        let ports_block = match c.container_port {
            Some(p) => format!(
                "      ports: {{\n        http: {{\n          containerPort: {p}\n        }}\n      }}\n"
            ),
            None => String::new(),
        };

        let mut env_lines = String::new();
        if !c.env.is_empty() || !ports_block.is_empty() {
            env_lines.push_str("      env: {\n");
            for (k, v) in &c.env {
                env_lines.push_str(&format!(
                    "        {}: {{ value: {} }}\n",
                    k,
                    bicep_env_value(k, v)
                ));
            }
            env_lines.push_str("      }\n");
        }

        let connections_block = if c.connections.is_empty() {
            String::new()
        } else {
            let mut s = String::from("    connections: {\n");
            for sym in &c.connections {
                // Lookup the resource kind for a stable connection key.
                let key = plan
                    .resources
                    .iter()
                    .find(|r| &r.symbol == sym)
                    .map(|r| r.kind.connection_key())
                    .unwrap_or("dep");
                s.push_str(&format!("      {key}: {{ source: {sym}.id }}\n"));
            }
            s.push_str("    }\n");
            s
        };

        out.push_str(&format!(
            "resource {sym} 'Applications.Core/containers@2023-10-01-preview' = {{\n  name: '{name}'\n  properties: {{\n    application: {app_symbol}.id\n    container: {{\n      image: {image_expr}\n{ports_block}{env_lines}    }}\n{connections_block}  }}\n}}\n\n",
            sym = sanitize_symbol(&c.name),
            name = c.name,
        ));
    }

    out
}

/// Map a docker-compose env value to a bicep `value:` expression. Values
/// referencing parameter-like names get hooked up to the well-known
/// parameters above (so generated files are immediately runnable).
fn bicep_env_value(key: &str, raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "''".into();
    }
    // Pass otel collector endpoint through the param.
    if key.contains("OTEL_EXPORTER_OTLP_ENDPOINT") {
        return "otelCollectorEndpoint".into();
    }
    // ${VAR:-default} or ${VAR} → drop interpolation, prefer default if present.
    if trimmed.starts_with("${") && trimmed.ends_with('}') {
        let inner = &trimmed[2..trimmed.len() - 1];
        if let Some((_, default)) = inner.split_once(":-") {
            return format!("'{}'", default.replace('\'', "\\'"));
        }
        return "''".into();
    }
    format!("'{}'", trimmed.replace('\'', "\\'"))
}

// ---------------------------------------------------------------------------
// LLM helpers
// ---------------------------------------------------------------------------

/// Build a small context bundle the LLM can use. Keeps total bytes
/// under `MAX_LLM_CONTEXT_BYTES` so we don't blow up the prompt window.
/// `compose_path` is optional — when absent (source-only modeling) the
/// bundle leans on manifests, Dockerfiles, and READMEs instead.
fn collect_context_bundle(input: &Path, compose_path: Option<&Path>) -> Result<String> {
    let mut bundle = String::new();
    let mut budget = MAX_LLM_CONTEXT_BYTES;

    if let Some(compose_path) = compose_path {
        append_file(&mut bundle, &mut budget, compose_path, input);
    }

    // Add likely-useful sibling signals: docs, env templates, and common
    // top-level package manifests (so source-only modeling has signal).
    let candidates = [
        "README.md",
        "readme.md",
        ".env.example",
        "package.json",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "Cargo.toml",
        "pom.xml",
        "build.gradle",
    ];
    for c in candidates {
        let p = input.join(c);
        if p.is_file() {
            append_file(&mut bundle, &mut budget, &p, input);
        }
    }

    // First-level Dockerfiles and package manifests per service folder.
    if let Ok(entries) = fs::read_dir(input) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                append_file(&mut bundle, &mut budget, &p.join("Dockerfile"), input);
                for manifest in ["package.json", "pyproject.toml", "go.mod", "Cargo.toml"] {
                    let m = p.join(manifest);
                    if m.is_file() {
                        append_file(&mut bundle, &mut budget, &m, input);
                    }
                }
                // C# project files.
                if let Ok(inner) = fs::read_dir(&p) {
                    for e in inner.flatten() {
                        let ip = e.path();
                        if ip.extension().and_then(|x| x.to_str()) == Some("csproj") {
                            append_file(&mut bundle, &mut budget, &ip, input);
                        }
                    }
                }
            }
        }
    }

    Ok(bundle)
}

fn append_file(bundle: &mut String, budget: &mut usize, path: &Path, root: &Path) {
    if *budget == 0 {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else { return };
    let rel = path.strip_prefix(root).unwrap_or(path).display().to_string();
    let header = format!("\n=== {rel} ===\n");
    let mut chunk = header;
    chunk.push_str(&text);
    if chunk.len() > *budget {
        chunk.truncate(*budget);
        chunk.push_str("\n…[truncated]…\n");
    }
    *budget = budget.saturating_sub(chunk.len());
    bundle.push_str(&chunk);
}

// ---------------------------------------------------------------------------
// Generation context + LLM client
// ---------------------------------------------------------------------------

/// Inputs shared by every generation strategy and the refine loop.
struct PackageContext {
    plan: Plan,
    /// Output of the deterministic compose analyzer — used directly by the
    /// `compose` strategy and as a seed/anchor for the LLM strategies.
    static_bicep: String,
    /// Collected source signals (compose file, Dockerfiles, READMEs).
    context_bundle: String,
    /// Repo-defined resource types, when available.
    catalog: Option<Catalog>,
    /// Existing "truth" app.bicep (`--truth-ref`) used as a baseline for
    /// grounding, regression-critiquing, and `--explain`.
    truth_ref: Option<String>,
}

/// Thin OpenAI-compatible chat client reused by LLM generators and the
/// LLM judge critic.
struct LlmClient {
    endpoint: String,
    model: String,
    api_key: String,
}

impl LlmClient {
    fn complete(&self, system: &str, user: &str, temperature: f64) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "temperature": temperature,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });

        let resp = ureq::post(&self.endpoint)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_json(body);

        let resp = match resp {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                bail!("LLM endpoint returned HTTP {code}: {body}");
            }
            Err(e) => bail!("LLM request failed: {e}"),
        };
        let v: serde_json::Value = resp.into_json().context("LLM response was not valid JSON")?;
        let content = v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("LLM response missing choices[0].message.content: {v}"))?
            .to_string();
        Ok(content)
    }
}

/// Shared user-prompt builder: analyzer plan + starter bicep + source
/// bundle, optionally followed by critic feedback for a refine pass.
fn build_user_prompt(
    plan: &Plan,
    static_bicep: &str,
    context_bundle: &str,
    catalog: Option<&Catalog>,
    truth_ref: Option<&str>,
    feedback: Option<&str>,
) -> String {
    let mut user = format!(
        "Application name: {name}\n\nAnalyzer plan:\n  resources: {res:?}\n  containers: {ctr:?}\n  skipped: {skip:?}\n\n--- Starter bicep (from static analyzer) ---\n{starter}\n\n--- Source bundle ---{bundle}\n",
        name = plan.app_name,
        res = plan.resources.iter().map(|r| format!("{}={}", r.name, r.kind.label())).collect::<Vec<_>>(),
        ctr = plan.containers.iter().map(|c| &c.name).collect::<Vec<_>>(),
        skip = plan.skipped,
        starter = static_bicep,
        bundle = context_bundle,
    );
    if let Some(cat) = catalog {
        user.push_str(&format!("\n--- Available platform resource types ---\n{}\n", cat.render_summary()));
    }
    if let Some(reference) = truth_ref {
        user.push_str(&format!(
            "\n--- Truth reference (existing app.bicep — authoritative baseline) ---\n\
This is the known-good definition for this application. Treat it as the source of \
truth for structure, resource naming, api-versions, and connection wiring. \
Preserve everything that still applies; change ONLY what the source bundle and \
plan require (e.g. new/removed services, updated images/ports). Do not rename or \
restructure resources gratuitously.\n{reference}\n"
        ));
    }
    if let Some(fb) = feedback {
        user.push_str(&format!(
            "\n--- Revision required ---\nYour previous output did NOT pass review. Fix every issue below and output the corrected bicep ONLY:\n{fb}\n"
        ));
    }
    user
}

// ---------------------------------------------------------------------------
// Generation strategies (the "strategy" pattern)
// ---------------------------------------------------------------------------

/// A generation strategy. New strategies implement this trait and are
/// dispatched from [`run`] via the [`Strategy`] enum.
trait Generator {
    /// Produce a candidate `app.bicep`. `feedback` carries critic issues
    /// on refine passes; generators that cannot act on feedback ignore it.
    fn generate(&self, ctx: &PackageContext, feedback: Option<&str>) -> Result<String>;

    /// The skill instructions backing this generator, if any. Used so the
    /// LLM judge can score against the skill's own checklist.
    fn skill_instructions(&self) -> Option<&str> {
        None
    }
}

/// Deterministic docker-compose analyzer. Ignores feedback (its output is
/// a pure function of the compose plan).
struct ComposeGenerator;

impl Generator for ComposeGenerator {
    fn generate(&self, ctx: &PackageContext, _feedback: Option<&str>) -> Result<String> {
        Ok(ctx.static_bicep.clone())
    }
}

/// Single-shot LLM seeded with the analyzer plan and starter bicep.
struct LlmGenerator {
    client: LlmClient,
}

impl Generator for LlmGenerator {
    fn generate(&self, ctx: &PackageContext, feedback: Option<&str>) -> Result<String> {
        let system = "You are a senior Radius (radapp.io) infrastructure engineer. \
Generate a complete, deployable Radius app.bicep file. Output ONLY bicep — no \
prose, no markdown fences. Prefer Radius custom resource types \
(Radius.Resources/postgreSqlDatabases, Radius.Resources/mqttBrokers, \
Radius.Resources/aiModels, Radius.Resources/workloadIdentities) over inline \
infrastructure when a service matches. Wire container `connections` to those \
resources. Skip observability infra (otel-collector, prometheus, zipkin, \
grafana) — those are provided by the platform portfolio. Use the provided \
analyzer plan and starter bicep as the source of truth; refine env vars, \
ports, and connections from the source bundle.";
        let user = build_user_prompt(
            &ctx.plan,
            &ctx.static_bicep,
            &ctx.context_bundle,
            ctx.catalog.as_ref(),
            ctx.truth_ref.as_deref(),
            feedback,
        );
        Ok(extract_bicep(&self.client.complete(system, &user, 0.1)?))
    }
}

/// LLM driven by an external agent-skill pack. The skill body becomes the
/// authoritative ruleset; its conversational "Output Format" / pull-request
/// choreography is explicitly neutralized so we get only bicep back.
struct SkillGenerator {
    client: LlmClient,
    pack: skill::SkillPack,
}

impl Generator for SkillGenerator {
    fn generate(&self, ctx: &PackageContext, feedback: Option<&str>) -> Result<String> {
        let system = format!(
            "You are an agent following the Radius app-modeling skill below as your \
authoritative ruleset (resource types, naming, structure, validation). \
IMPORTANT OVERRIDES: ignore any part of the skill describing chat output format, \
numbered conversational steps, blockquotes, or creating a pull request. Do NOT \
narrate. Output ONLY the final `.radius/app.bicep` contents — no markdown fences, \
no prose. Skip observability infra (otel-collector, prometheus, zipkin, grafana).\n\
IMPORTANT: a list of available platform resource types is provided in the user \
message. Use ONLY those resource types and their exact api-versions; ignore any \
other type registry (e.g. resource-types-contrib) the skill instructs you to read.\n\n\
========== SKILL: {repo}/{name} ==========\n{body}\n========== END SKILL ==========",
            repo = self.pack.repo,
            name = self.pack.skill,
            body = self.pack.instructions,
        );
        let user = build_user_prompt(
            &ctx.plan,
            &ctx.static_bicep,
            &ctx.context_bundle,
            ctx.catalog.as_ref(),
            ctx.truth_ref.as_deref(),
            feedback,
        );
        Ok(extract_bicep(&self.client.complete(&system, &user, 0.1)?))
    }

    fn skill_instructions(&self) -> Option<&str> {
        Some(&self.pack.instructions)
    }
}

// ---------------------------------------------------------------------------
// Critics (generate → critique → refine)
// ---------------------------------------------------------------------------

/// Result of evaluating a candidate against one critic.
struct Critique {
    pass: bool,
    issues: Vec<String>,
}

/// A quality check applied to a candidate `app.bicep`.
trait Critic {
    fn label(&self) -> &str;
    fn critique(&self, bicep: &str, ctx: &PackageContext) -> Result<Critique>;
}

/// Deterministic grounding critic. Rejects any `<namespace>/<type>@<ver>`
/// reference whose type or api-version is not declared in the repo's
/// resource-type catalog. Cheap and high-value: it catches hallucinated
/// types and stale api-versions before the (slower) compile critic runs.
struct CatalogCritic;

impl Critic for CatalogCritic {
    fn label(&self) -> &str {
        "resource-type catalog"
    }

    fn critique(&self, bicep: &str, ctx: &PackageContext) -> Result<Critique> {
        let Some(cat) = ctx.catalog.as_ref() else {
            return Ok(Critique { pass: true, issues: Vec::new() });
        };
        let mut issues = Vec::new();
        for (type_name, version) in catalog::referenced_types(bicep, &cat.namespace) {
            if !cat.has_type(&type_name) {
                let known: Vec<&str> = cat.types.iter().map(|t| t.name.as_str()).collect();
                issues.push(format!(
                    "unknown type `{}/{}` — valid types: {}",
                    cat.namespace,
                    type_name,
                    known.join(", ")
                ));
            } else if !version.is_empty() && !cat.has(&type_name, &version) {
                let want = cat
                    .type_ref(&type_name)
                    .unwrap_or_else(|| type_name.clone());
                issues.push(format!(
                    "`{}/{}` uses api-version `{}` but the catalog defines `{}`",
                    cat.namespace, type_name, version, want
                ));
            }
        }
        // De-duplicate (the same bad type may appear on several resources).
        issues.sort();
        issues.dedup();
        Ok(Critique { pass: issues.is_empty(), issues })
    }
}

/// Deterministic compile critic. Runs `rad`/`bicep`/`az` to build the
/// candidate; a non-zero exit surfaces the compiler diagnostics as issues.
struct BicepBuildCritic {
    tool: PathBuf,
    kind: BicepTool,
}

#[derive(Clone, Copy)]
enum BicepTool {
    /// `rad bicep build <file>`
    Rad,
    /// `bicep build <file>`
    Bicep,
    /// `az bicep build --file <file>`
    Az,
}

impl BicepBuildCritic {
    /// Find a bicep-capable tool on PATH, preferring `rad` (bundles the
    /// Radius extension) then standalone `bicep`, then `az`.
    fn discover() -> Option<Self> {
        for (bin, kind) in [
            ("rad", BicepTool::Rad),
            ("bicep", BicepTool::Bicep),
            ("az", BicepTool::Az),
        ] {
            if let Ok(path) = which::which(bin) {
                return Some(Self { tool: path, kind });
            }
        }
        None
    }
}

impl Critic for BicepBuildCritic {
    fn label(&self) -> &str {
        match self.kind {
            BicepTool::Rad => "rad bicep build",
            BicepTool::Bicep => "bicep build",
            BicepTool::Az => "az bicep build",
        }
    }

    fn critique(&self, bicep: &str, _ctx: &PackageContext) -> Result<Critique> {
        // Build in an isolated temp dir alongside the bundled bicepconfig
        // (which registers the Radius extensions) when it is available.
        let dir = std::env::temp_dir().join(format!("ada-package-{}", std::process::id()));
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create temp dir {}", dir.display()))?;
        let bicep_path = dir.join("app.bicep");
        fs::write(&bicep_path, bicep)
            .with_context(|| format!("failed to write {}", bicep_path.display()))?;
        if let Ok(radius_root) = home::radius_root() {
            let cfg = radius_root.join("bicepconfig.json");
            if cfg.is_file() {
                let _ = fs::copy(&cfg, dir.join("bicepconfig.json"));
            }
        }

        let mut cmd = Command::new(&self.tool);
        match self.kind {
            BicepTool::Rad => {
                cmd.arg("bicep").arg("build").arg(&bicep_path);
            }
            BicepTool::Bicep => {
                cmd.arg("build").arg(&bicep_path);
            }
            BicepTool::Az => {
                cmd.arg("bicep").arg("build").arg("--file").arg(&bicep_path);
            }
        }
        cmd.current_dir(&dir);

        let output = cmd
            .output()
            .with_context(|| format!("failed to run {}", self.label()))?;
        if output.status.success() {
            return Ok(Critique { pass: true, issues: Vec::new() });
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let issues: Vec<String> = stderr
            .lines()
            .filter(|l| {
                let l = l.to_lowercase();
                l.contains("error") || l.contains("warning")
            })
            .map(|l| l.trim().to_string())
            .take(40)
            .collect();
        let issues = if issues.is_empty() {
            vec![format!("{} exited with {}", self.label(), output.status)]
        } else {
            issues
        };
        Ok(Critique { pass: false, issues })
    }
}

/// LLM-as-judge critic. Scores the candidate against the skill's own
/// validation checklist (or a built-in rubric) and returns concrete issues.
struct ChecklistCritic {
    client: LlmClient,
    rubric: String,
}

impl Critic for ChecklistCritic {
    fn label(&self) -> &str {
        "llm checklist judge"
    }

    fn critique(&self, bicep: &str, _ctx: &PackageContext) -> Result<Critique> {
        let system = "You are a strict reviewer of Radius app.bicep files. Evaluate the \
candidate against the provided rubric. Respond with ONLY a JSON object of the form \
{\"pass\": bool, \"issues\": [\"...\"]}. `pass` is true only if there are no blocking \
issues. List each concrete, actionable problem as a short string. No prose outside \
the JSON.";
        let user = format!(
            "--- Rubric ---\n{rubric}\n\n--- Candidate app.bicep ---\n{bicep}\n",
            rubric = self.rubric,
        );
        let raw = self.client.complete(system, &user, 0.0)?;
        Ok(parse_judge_json(&raw))
    }
}

/// Parse the judge's JSON verdict defensively. On any parse failure we
/// fail open (pass = true) so a flaky judge never blocks output.
fn parse_judge_json(raw: &str) -> Critique {
    let slice = match (raw.find('{'), raw.rfind('}')) {
        (Some(a), Some(b)) if b > a => &raw[a..=b],
        _ => return Critique { pass: true, issues: Vec::new() },
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(slice) else {
        return Critique { pass: true, issues: Vec::new() };
    };
    let pass = v.get("pass").and_then(|p| p.as_bool()).unwrap_or(true);
    let issues = v
        .get("issues")
        .and_then(|i| i.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Critique { pass: pass && issues.is_empty(), issues }
}

/// Fallback rubric used by the LLM judge when no skill pack is loaded.
const BUILTIN_RUBRIC: &str = "\
- Exactly one Applications.Core/applications@2023-10-01-preview resource.
- `param environment string` is declared and referenced by every resource.
- Radius.* resource types use api-version 2025-08-01-preview.
- `connections` is a top-level object map under `properties`, never an array, never inside `containers`.
- Container ports use `containerPort`, not `port`.
- No secrets or passwords hardcoded; use @secure() params.
- No markdown fences, comments-only files, or prose — bicep only.
- Observability infra (otel-collector, prometheus, grafana, zipkin) is NOT modeled.";

/// LLM critic for update scenarios. Compares the candidate against the
/// `--truth-ref` baseline and flags *regressions* — resources, connections,
/// or settings that were dropped or altered without a source-driven reason.
/// Intentional changes (new/removed services reflected in the source) are
/// allowed. Fails open on a bad verdict so a flaky judge never blocks output.
struct TruthRefCritic {
    client: LlmClient,
}

impl Critic for TruthRefCritic {
    fn label(&self) -> &str {
        "truth-ref regression"
    }

    fn critique(&self, bicep: &str, ctx: &PackageContext) -> Result<Critique> {
        let Some(reference) = ctx.truth_ref.as_deref() else {
            return Ok(Critique { pass: true, issues: Vec::new() });
        };
        let system = "You are reviewing an updated Radius app.bicep against a known-good \
baseline. Report only REGRESSIONS: resources, connections, params, ports, or \
settings present in the baseline that the candidate dropped or changed WITHOUT a \
justification visible in the candidate (e.g. a removed service). Additions and \
intentional updates are NOT regressions. Respond with ONLY a JSON object \
{\"pass\": bool, \"issues\": [\"...\"]}; `pass` is true when there are no regressions. \
No prose outside the JSON.";
        let user = format!(
            "--- Baseline (truth reference) ---\n{reference}\n\n--- Candidate ---\n{bicep}\n"
        );
        let raw = self.client.complete(system, &user, 0.0)?;
        Ok(parse_judge_json(&raw))
    }
}

/// Produce a human-readable explanation of how the generated bicep differs
/// from the truth reference and why. Used by `--explain` for
/// update/iteration scenarios.
fn explain_divergence(client: &LlmClient, reference: &str, generated: &str) -> Result<String> {
    let system = "You are a Radius infrastructure reviewer. Compare an updated app.bicep \
against its baseline and explain the differences for a human reviewer. Be concise \
and concrete. Use these sections with markdown bullets (omit a section if empty):\n\
## Added\n## Removed\n## Changed\n## Risk / review notes\n\
For each item, name the resource/property and briefly say WHY the change likely \
happened (e.g. inferred from the source bundle, naming-convention fix, dropped \
hardcoded secret). Do NOT output bicep.";
    let user = format!(
        "--- Baseline (truth reference) ---\n{reference}\n\n--- Generated ---\n{generated}\n"
    );
    client.complete(system, &user, 0.2)
}

// ---------------------------------------------------------------------------
// Refine loop
// ---------------------------------------------------------------------------

/// Generate → critique → refine until all critics pass or `max_iters` is
/// reached. Always returns the most recent candidate so output is never
/// lost to an unsatisfiable critic (a warning is emitted instead).
fn refine(
    generator: &dyn Generator,
    critics: &[Box<dyn Critic>],
    ctx: &PackageContext,
    max_iters: u32,
) -> Result<String> {
    let max_iters = max_iters.max(1);
    let mut feedback: Option<String> = None;
    let mut candidate = String::new();

    for iter in 1..=max_iters {
        ui::step(&format!("iteration {iter}/{max_iters}: generating candidate"));
        candidate = generator.generate(ctx, feedback.as_deref())?;

        let mut all_issues: Vec<String> = Vec::new();
        for critic in critics {
            let verdict = critic.critique(&candidate, ctx)?;
            if verdict.pass {
                ui::ok(&format!("critic [{}] passed", critic.label()));
            } else {
                ui::warn(&format!(
                    "critic [{}] found {} issue(s)",
                    critic.label(),
                    verdict.issues.len()
                ));
                for issue in &verdict.issues {
                    ui::bullet(&format!("  - {issue}"));
                    all_issues.push(format!("[{}] {issue}", critic.label()));
                }
            }
        }

        if all_issues.is_empty() {
            ui::ok(&format!("all critics passed on iteration {iter}"));
            return Ok(candidate);
        }
        if iter < max_iters {
            feedback = Some(all_issues.join("\n"));
        } else {
            ui::warn(&format!(
                "critics still reported {} issue(s) after {max_iters} iteration(s); emitting best candidate",
                all_issues.len()
            ));
        }
    }
    Ok(candidate)
}


/// LLMs often wrap code in ```bicep … ``` fences despite instructions.
/// Strip them if present.
/// Extract the Bicep template from a model response, discarding any
/// surrounding prose or markdown. Models (especially skill-driven ones)
/// often narrate before/after the template and wrap it in a ```bicep
/// fence. This handles all of: a fence anywhere in the response, bare
/// prose around a template, or already-clean bicep. Applied to every
/// strategy's output so the artifact never carries stray text.
fn extract_bicep(s: &str) -> String {
    let text = s.trim();

    // 1) Prefer a fenced code block if one appears anywhere in the text.
    if let Some(open) = text.find("```") {
        let after_open = &text[open + 3..];
        // Drop the optional language tag on the remainder of the fence line.
        let body = after_open
            .split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or("");
        let body = match body.find("```") {
            Some(close) => &body[..close],
            None => body,
        };
        let cleaned = strip_prose(body);
        if !cleaned.is_empty() {
            return cleaned;
        }
    }

    // 2) No usable fence — trim leading/trailing prose from a bare template.
    strip_prose(text)
}

/// Trim leading and trailing non-Bicep prose lines from a template body,
/// keeping everything from the first Bicep token to the final closing
/// brace.
fn strip_prose(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let is_bicep_start = |l: &&str| {
        let t = l.trim_start();
        t.starts_with("extension ")
            || t.starts_with("import ")
            || t.starts_with("targetScope")
            || t.starts_with("metadata ")
            || t.starts_with("param ")
            || t.starts_with("var ")
            || t.starts_with("resource ")
            || t.starts_with("module ")
            || t.starts_with("output ")
            || t.starts_with("type ")
            || t.starts_with("func ")
            || t.starts_with('@')
            || t.starts_with("//")
    };
    let Some(start) = lines.iter().position(is_bicep_start) else {
        return body.trim().to_string();
    };
    // The generated app.bicep always ends in a closing brace; keep up to the
    // last one so trailing prose (e.g. "Would you like a pull request?") is
    // dropped. Fall back to end-of-text if there is no brace.
    let end = lines
        .iter()
        .rposition(|l| l.trim_end().ends_with('}'))
        .unwrap_or(lines.len().saturating_sub(1))
        .max(start);
    lines[start..=end].join("\n").trim_end().to_string()
}
