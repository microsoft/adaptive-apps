//! `ada package` — analyze a source folder and emit a Radius app
//! definition (`app.bicep`).
//!
//! Two modes are available:
//!
//! * **Static** (default): parse `docker-compose.yml` from the input
//!   folder and map services to Radius resources using a small ruleset
//!   (postgres → `postgreSqlDatabases`, mosquitto/MQTT → `mqttBrokers`,
//!   observability infra → skipped, everything else →
//!   `Applications.Core/containers`).
//! * **LLM-assisted** (`--llm`): collect the docker-compose file plus a
//!   short bundle of source signals (Dockerfiles, READMEs, manifests)
//!   and ask an OpenAI-compatible chat completion endpoint to emit a
//!   bicep file. The static result is included in the prompt as a
//!   starting point. Requires an API key.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use serde::Deserialize;

use crate::ui;

const MAX_LLM_CONTEXT_BYTES: usize = 32 * 1024;
const DEFAULT_LLM_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_LLM_MODEL: &str = "gpt-4o-mini";

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

    /// Use an LLM to refine the bicep instead of emitting the static
    /// analyzer output directly. Requires `--llm-api-key` or
    /// `OPENAI_API_KEY` in the environment.
    #[arg(long)]
    llm: bool,

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

    ui::heading("Package");
    ui::detail("input", &args.input.display().to_string());
    ui::detail("output", &args.output.display().to_string());
    ui::detail("app name", &app_name);
    ui::detail("mode", if args.llm { "llm-assisted" } else { "static" });

    let compose_path = find_compose(&args.input)?;
    ui::detail("compose", &compose_path.display().to_string());

    ui::step("analyzing docker-compose services");
    let plan = analyze_compose(&compose_path, &app_name)?;
    summarize_plan(&plan);

    let static_bicep = render_bicep(&plan);

    let final_bicep = if args.llm {
        let api_key = args
            .llm_api_key
            .clone()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                anyhow!("--llm requires an API key; set OPENAI_API_KEY or pass --llm-api-key")
            })?;
        ui::step(&format!("calling LLM ({})", args.llm_model));
        let context_bundle = collect_context_bundle(&args.input, &compose_path)?;
        ui::tool_banner("llm", &format!("POST {}", args.llm_endpoint));
        let result = call_llm(
            &args.llm_endpoint,
            &args.llm_model,
            &api_key,
            &plan,
            &static_bicep,
            &context_bundle,
        )?;
        ui::ok("LLM response received");
        result
    } else {
        static_bicep
    };

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

fn find_compose(dir: &Path) -> Result<PathBuf> {
    for candidate in ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"] {
        let p = dir.join(candidate);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(anyhow!(
        "no docker-compose file found under {} (looked for docker-compose.yml/yaml, compose.yml/yaml)",
        dir.display()
    ))
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

fn render_bicep(plan: &Plan) -> String {
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
        out.push_str(&format!(
            "resource {} '{}' = {{\n  name: '{}'\n  properties: {{\n    environment: environment\n    application: {app_symbol}.id\n{extra}  }}\n}}\n\n",
            r.symbol, r.kind.type_ref(), r.name,
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
fn collect_context_bundle(input: &Path, compose_path: &Path) -> Result<String> {
    let mut bundle = String::new();
    let mut budget = MAX_LLM_CONTEXT_BYTES;

    append_file(&mut bundle, &mut budget, compose_path, input);

    // Add likely-useful sibling signals.
    let candidates = [
        "README.md",
        "readme.md",
        ".env.example",
    ];
    for c in candidates {
        let p = input.join(c);
        if p.is_file() {
            append_file(&mut bundle, &mut budget, &p, input);
        }
    }

    // First-level Dockerfiles per service folder.
    if let Ok(entries) = fs::read_dir(input) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let dockerfile = p.join("Dockerfile");
                if dockerfile.is_file() {
                    append_file(&mut bundle, &mut budget, &dockerfile, input);
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

fn call_llm(
    endpoint: &str,
    model: &str,
    api_key: &str,
    plan: &Plan,
    static_bicep: &str,
    context_bundle: &str,
) -> Result<String> {
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

    let user = format!(
        "Application name: {name}\n\nAnalyzer plan:\n  resources: {res:?}\n  containers: {ctr:?}\n  skipped: {skip:?}\n\n--- Starter bicep (from static analyzer) ---\n{starter}\n\n--- Source bundle ---{bundle}\n",
        name = plan.app_name,
        res = plan.resources.iter().map(|r| format!("{}={}", r.name, r.kind.label())).collect::<Vec<_>>(),
        ctr = plan.containers.iter().map(|c| &c.name).collect::<Vec<_>>(),
        skip = plan.skipped,
        starter = static_bicep,
        bundle = context_bundle,
    );

    let body = serde_json::json!({
        "model": model,
        "temperature": 0.1,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });

    let resp = ureq::post(endpoint)
        .set("Authorization", &format!("Bearer {api_key}"))
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
    let v: serde_json::Value = resp
        .into_json()
        .context("LLM response was not valid JSON")?;
    let content = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|s| s.as_str())
        .ok_or_else(|| anyhow!("LLM response missing choices[0].message.content: {v}"))?
        .to_string();

    Ok(strip_code_fences(&content))
}

/// LLMs often wrap code in ```bicep … ``` fences despite instructions.
/// Strip them if present.
fn strip_code_fences(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Drop the language tag on the first line.
        let after_first_line = rest.split_once('\n').map(|(_, r)| r).unwrap_or(rest);
        if let Some(inner) = after_first_line.strip_suffix("```") {
            return inner.trim_end().to_string();
        }
        if let Some(inner) = after_first_line.rsplit_once("```").map(|(a, _)| a) {
            return inner.trim_end().to_string();
        }
    }
    trimmed.to_string()
}
