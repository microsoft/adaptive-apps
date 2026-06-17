//! `ada oidc` — provision an OIDC client in a chart-deployed Keycloak.
//!
//! Replaces the manual "open the Keycloak admin console → create a client →
//! copy the secret" steps in the getting-started tutorials with a single
//! API-driven flow. Also reusable from `ada bootstrap` so the full
//! install path can be automated end-to-end.
//!
//! Reachability strategy: spawn a short-lived `kubectl port-forward` to
//! the keycloak Service, talk to the Admin REST API on localhost, then
//! kill it. This avoids any dependency on what's baked into the
//! Keycloak container image and works against any cluster the user's
//! current kubectl context can reach.

use std::io::Read;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};

use crate::ui;

#[derive(Debug, Args)]
pub struct OidcArgs {
    #[command(subcommand)]
    command: OidcCommand,
}

#[derive(Debug, Subcommand)]
enum OidcCommand {
    /// Create (or reuse) an OIDC client in a chart-deployed Keycloak
    /// realm and print `export OIDC_CLIENT_ID=… OIDC_CLIENT_SECRET=…`
    /// lines suitable for `eval`.
    SetupClient(SetupClientArgs),
}

#[derive(Debug, Args, Clone)]
pub struct SetupClientArgs {
    /// Namespace where the Keycloak chart is installed.
    #[arg(long, short = 'n')]
    pub namespace: String,

    /// Helm release name. The Keycloak Service is expected at
    /// `<release>-keycloak` on port 8080.
    #[arg(long)]
    pub release: String,

    /// OIDC clientId to create. Reused if it already exists.
    #[arg(long, default_value = "adaptive-apps")]
    pub client_id: String,

    /// Valid redirect URIs (repeatable). Defaults to the tutorial value.
    #[arg(long = "redirect-uri", default_values_t = vec!["http://localhost:3000/*".to_string()])]
    pub redirect_uris: Vec<String>,

    /// Allowed web origins (repeatable). Defaults to the frontend origin.
    #[arg(long = "web-origin", default_values_t = vec!["http://localhost:3000".to_string()])]
    pub web_origins: Vec<String>,

    /// Keycloak realm. Defaults to `master` (the realm the chart provisions).
    #[arg(long, default_value = "master")]
    pub realm: String,

    /// Keycloak admin username. Defaults to the chart's default (`admin`).
    #[arg(long, default_value = "admin")]
    pub admin_user: String,

    /// Keycloak admin password. Defaults to the chart's default (`admin`).
    /// If the operator rotated the admin password, pass the current
    /// value (or read it from the chart's secret).
    #[arg(long, default_value = "admin")]
    pub admin_password: String,

    /// Local port to bind for the kubectl port-forward to Keycloak.
    /// Defaults to 8080 so the browser-facing `OIDC_BROWSER_AUTH_ENDPOINT`
    /// matches what the deployed app expects without extra configuration.
    #[arg(long, default_value_t = 8080)]
    pub local_port: u16,

    /// How long to wait for the Keycloak pod to become Ready before
    /// starting the port-forward.
    #[arg(long, default_value = "180")]
    pub pod_ready_timeout_secs: u64,

    /// Keep the `kubectl port-forward` to Keycloak running after the
    /// client has been provisioned. Useful when you also want to open
    /// the Keycloak admin UI in a browser right after. The forward
    /// runs in the foreground; press Ctrl-C to stop it.
    #[arg(long)]
    pub keep_port_forward: bool,

    /// Print what would happen without provisioning the client.
    #[arg(long)]
    pub dry_run: bool,
}

/// Outcome of a successful client provisioning.
pub struct ProvisionedClient {
    pub client_id: String,
    pub client_secret: String,
    /// True when the clientId already existed and we just fetched its
    /// secret. False when we created the client just now.
    #[allow(dead_code)]
    pub reused: bool,
}

pub fn run(args: OidcArgs) -> Result<()> {
    match args.command {
        OidcCommand::SetupClient(a) => {
            let (result, pf) = provision_client_keep(&a)?;
            print_exports(&a, &result);
            if let Some(mut pf) = pf {
                println!();
                ui::note(&format!(
                    "keeping port-forward to svc/{}-keycloak on http://localhost:{} — press Ctrl-C to stop",
                    a.release, a.local_port
                ));
                let _ = pf.wait();
            }
            Ok(())
        }
    }
}

/// Reusable from `bootstrap`. Returns the credentials of the
/// provisioned (or reused) client. Idempotent. Always tears down the
/// port-forward before returning.
#[allow(dead_code)]
pub fn provision_client(args: &SetupClientArgs) -> Result<ProvisionedClient> {
    let (result, _pf) = provision_client_keep(args)?;
    // _pf drops here, killing the port-forward.
    Ok(result)
}

/// Like [`provision_client`] but returns the live `PortForward` when
/// `args.keep_port_forward` is set, so the caller can hand control back
/// to the user (e.g. block on Ctrl-C).
pub fn provision_client_keep(
    args: &SetupClientArgs,
) -> Result<(ProvisionedClient, Option<PortForward>)> {
    which::which("kubectl")
        .map_err(|_| anyhow!("`kubectl` not found on PATH; install it and re-run"))?;
    let service = format!("{}-keycloak", args.release);

    ui::heading("OIDC client provisioning");
    ui::detail("namespace", &args.namespace);
    ui::detail("service", &format!("svc/{service}:8080"));
    ui::detail("realm", &args.realm);
    ui::detail("clientId", &args.client_id);
    ui::detail("redirect URIs", &args.redirect_uris.join(", "));

    if args.dry_run {
        ui::dry_run("would port-forward to keycloak and create/reuse the client via Admin REST API");
        return Ok((
            ProvisionedClient {
                client_id: args.client_id.clone(),
                client_secret: "<dry-run>".into(),
                reused: false,
            },
            None,
        ));
    }

    wait_for_pod_ready(
        &args.namespace,
        &args.release,
        Duration::from_secs(args.pod_ready_timeout_secs),
    )?;

    let local_port = args.local_port;
    let mut pf = PortForward::start(&args.namespace, &service, local_port)?;
    let base = format!("http://127.0.0.1:{local_port}");

    if let Err(e) = wait_for_keycloak(&base, &args.realm, Duration::from_secs(30)) {
        if let Some(extra) = pf.diagnostics() {
            bail!("{e}\n\nkubectl port-forward output:\n{extra}");
        }
        return Err(e);
    }

    let token = admin_token(&base, &args.admin_user, &args.admin_password)?;
    let result = ensure_client(&base, &args.realm, &token, args)?;
    let pf = if args.keep_port_forward { Some(pf) } else { None };
    Ok((result, pf))
}

/// Print `export …` lines for the caller's shell. Used by both the
/// subcommand and (for reference) by bootstrap.
///
/// Also reads the chart's `<release>-oidc` ConfigMap so callers get the
/// full set of in-cluster endpoint URLs (issuer/auth/token/userinfo) ─
/// not just the browser-facing localhost URL. Without these, the
/// frontend pod can't talk to Keycloak for the token exchange and the
/// OIDC callback fails with ECONNREFUSED on `localhost:8080`.
pub fn print_exports(args: &SetupClientArgs, result: &ProvisionedClient) {
    println!();
    ui::note("OIDC client ready. To use with `rad deploy`, run:");
    println!("export OIDC_CLIENT_ID={}", result.client_id);
    println!("export OIDC_CLIENT_SECRET={}", result.client_secret);

    let cm_name = format!("{}-oidc", args.release);
    match read_oidc_configmap(&args.namespace, &cm_name) {
        Ok(cm) => {
            println!("export OIDC_ISSUER={}", cm.issuer);
            println!("export OIDC_AUTH_ENDPOINT={}", cm.auth_endpoint);
            println!("export OIDC_TOKEN_ENDPOINT={}", cm.token_endpoint);
            println!("export OIDC_USERINFO_ENDPOINT={}", cm.userinfo_endpoint);
        }
        Err(e) => {
            ui::note(&format!(
                "warning: couldn't read in-cluster OIDC endpoints from cm/{cm_name}: {e}"
            ));
            ui::note("the frontend pod will not be able to reach Keycloak for token exchange unless you set OIDC_ISSUER / OIDC_TOKEN_ENDPOINT / OIDC_USERINFO_ENDPOINT manually.");
        }
    }
    println!(
        "export OIDC_BROWSER_AUTH_ENDPOINT=http://localhost:8080/realms/{}/protocol/openid-connect/auth",
        args.realm
    );
}

struct OidcConfigMap {
    issuer: String,
    auth_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
}

fn read_oidc_configmap(namespace: &str, name: &str) -> Result<OidcConfigMap> {
    let out = Command::new("kubectl")
        .args([
            "get", "cm", "-n", namespace, name,
            "-o", "jsonpath={.data.issuer}|{.data.authEndpoint}|{.data.tokenEndpoint}|{.data.userInfoEndpoint}",
        ])
        .output()
        .with_context(|| format!("failed to run kubectl get cm {name}"))?;
    if !out.status.success() {
        bail!(
            "kubectl get cm {name}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let body = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let parts: Vec<&str> = body.split('|').collect();
    if parts.len() != 4 || parts.iter().any(|p| p.is_empty()) {
        bail!("cm/{name} did not expose all 4 endpoint keys: `{body}`");
    }
    Ok(OidcConfigMap {
        issuer: parts[0].to_string(),
        auth_endpoint: parts[1].to_string(),
        token_endpoint: parts[2].to_string(),
        userinfo_endpoint: parts[3].to_string(),
    })
}

// ---------------------------------------------------------------------------
// kubectl port-forward lifecycle
// ---------------------------------------------------------------------------

/// Drops kill the spawned `kubectl port-forward` child process.
pub struct PortForward {
    child: Child,
    stderr: Option<ChildStderr>,
}

impl PortForward {
    fn start(namespace: &str, service: &str, local_port: u16) -> Result<Self> {
        ui::step(&format!(
            "kubectl port-forward -n {namespace} svc/{service} {local_port}:8080"
        ));
        let mut child = Command::new("kubectl")
            .args([
                "port-forward",
                "-n",
                namespace,
                &format!("svc/{service}"),
                &format!("{local_port}:8080"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!(
                "failed to spawn `kubectl port-forward` on local port {local_port}"
            ))?;
        let stderr = child.stderr.take();
        Ok(Self { child, stderr })
    }

    /// If the child has exited (or has output queued on stderr), drain
    /// and return what `kubectl` reported. Used to give the user the
    /// actual reason port-forwarding failed instead of just the
    /// downstream HTTP connection refused.
    fn diagnostics(&mut self) -> Option<String> {
        let mut buf = String::new();
        if let Some(mut s) = self.stderr.take() {
            let _ = s.read_to_string(&mut buf);
        }
        let exit = self.child.try_wait().ok().flatten();
        let trimmed = buf.trim();
        if trimmed.is_empty() && exit.is_none() {
            return None;
        }
        let mut out = String::new();
        if !trimmed.is_empty() {
            out.push_str(trimmed);
        }
        if let Some(status) = exit {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("(kubectl exited: {status})"));
        }
        Some(out)
    }
    /// Block until the child exits (e.g. user hits Ctrl-C).
    pub fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait()
    }
}

impl Drop for PortForward {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Block until the Keycloak Service has at least one Ready pod, so the
/// subsequent `kubectl port-forward` actually has an endpoint to attach
/// to. Keycloak's initial boot typically takes 60-90s on a fresh
/// cluster.
fn wait_for_pod_ready(namespace: &str, release: &str, timeout: Duration) -> Result<()> {
    let service = format!("{release}-keycloak");
    let selector = service_selector(namespace, &service)?;
    ui::step(&format!(
        "waiting up to {}s for keycloak pod (selector `{selector}`) to be Ready",
        timeout.as_secs()
    ));
    let deadline = Instant::now() + timeout;

    // 1. Poll until at least one pod matches the selector. `kubectl wait`
    //    returns immediately ("no matching resources found") when zero
    //    pods exist, so we can't rely on it to wait for pod creation.
    loop {
        let out = Command::new("kubectl")
            .args([
                "get", "pod", "-n", namespace, "-l", &selector,
                "-o", "name",
            ])
            .output()
            .context("failed to run `kubectl get pod` for keycloak")?;
        if out.status.success() && !out.stdout.is_empty() {
            break;
        }
        if Instant::now() >= deadline {
            bail!(
                "no keycloak pod (selector `{selector}`) appeared in namespace `{namespace}` within {}s",
                timeout.as_secs()
            );
        }
        thread::sleep(Duration::from_secs(2));
    }

    // 2. Now that at least one pod exists, wait for Ready with whatever
    //    time remains.
    let remaining = deadline.saturating_duration_since(Instant::now());
    let remaining_secs = remaining.as_secs().max(1);
    let out = Command::new("kubectl")
        .args([
            "wait",
            "-n",
            namespace,
            "--for=condition=Ready",
            "pod",
            "-l",
            &selector,
            &format!("--timeout={remaining_secs}s"),
        ])
        .output()
        .context("failed to run `kubectl wait` for keycloak pod")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!(
            "keycloak pod did not become Ready in namespace `{namespace}` within {}s: {}",
            timeout.as_secs(),
            stderr.trim()
        );
    }
    Ok(())
}

/// Read the Service's `spec.selector` and format it as a `key=value,...`
/// label selector string. This way we always target the exact set of
/// pods the Service routes to, regardless of which chart wrote them.
fn service_selector(namespace: &str, service: &str) -> Result<String> {
    let out = Command::new("kubectl")
        .args([
            "get", "svc", "-n", namespace, service,
            "-o", "jsonpath={.spec.selector}",
        ])
        .output()
        .with_context(|| format!("failed to read selector of svc/{service} in {namespace}"))?;
    if !out.status.success() {
        bail!(
            "svc/{service} not found in namespace `{namespace}`: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let body = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if body.is_empty() || body == "<nil>" {
        bail!("svc/{service} in namespace `{namespace}` has no spec.selector");
    }
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&body)
        .with_context(|| format!("could not parse selector JSON: {body}"))?;
    if map.is_empty() {
        bail!("svc/{service} in namespace `{namespace}` has an empty spec.selector");
    }
    let parts: Vec<String> = map
        .into_iter()
        .filter_map(|(k, v)| v.as_str().map(|s| format!("{k}={s}")))
        .collect();
    Ok(parts.join(","))
}

/// Poll the realm's OIDC discovery doc until it returns 200 (or timeout).
fn wait_for_keycloak(base: &str, realm: &str, timeout: Duration) -> Result<()> {
    let url = format!("{base}/realms/{realm}/.well-known/openid-configuration");
    let deadline = Instant::now() + timeout;
    let mut last_err: Option<String> = None;
    while Instant::now() < deadline {
        match ureq::get(&url).timeout(Duration::from_secs(2)).call() {
            Ok(_) => return Ok(()),
            Err(e) => last_err = Some(e.to_string()),
        }
        thread::sleep(Duration::from_millis(300));
    }
    bail!(
        "keycloak did not become reachable at {url} within {:?}: {}",
        timeout,
        last_err.unwrap_or_else(|| "<no error>".into())
    )
}

// ---------------------------------------------------------------------------
// Keycloak Admin REST API
// ---------------------------------------------------------------------------

fn admin_token(base: &str, user: &str, password: &str) -> Result<String> {
    let url = format!("{base}/realms/master/protocol/openid-connect/token");
    let resp = ureq::post(&url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_form(&[
            ("grant_type", "password"),
            ("client_id", "admin-cli"),
            ("username", user),
            ("password", password),
        ]);
    let body = match resp {
        Ok(r) => r.into_json::<serde_json::Value>()
            .context("admin-cli token response was not JSON")?,
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            bail!(
                "keycloak admin token request failed (HTTP {code}): {body}\n\
                 If you rotated the admin password, pass --admin-password."
            );
        }
        Err(e) => bail!("keycloak admin token request failed: {e}"),
    };
    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("admin token response missing access_token: {body}"))
}

fn ensure_client(
    base: &str,
    realm: &str,
    token: &str,
    args: &SetupClientArgs,
) -> Result<ProvisionedClient> {
    // 1. Look up existing client by clientId.
    if let Some(uuid) = find_client_uuid(base, realm, token, &args.client_id)? {
        ui::step(&format!(
            "client `{}` already exists, reusing (uuid {uuid})",
            args.client_id
        ));
        let secret = fetch_client_secret(base, realm, token, &uuid)?;
        return Ok(ProvisionedClient {
            client_id: args.client_id.clone(),
            client_secret: secret,
            reused: true,
        });
    }

    // 2. Create it.
    ui::step(&format!("creating client `{}`", args.client_id));
    let body = serde_json::json!({
        "clientId": args.client_id,
        "enabled": true,
        "protocol": "openid-connect",
        "publicClient": false,
        "standardFlowEnabled": true,
        "directAccessGrantsEnabled": false,
        "serviceAccountsEnabled": false,
        "redirectUris": args.redirect_uris,
        "webOrigins": args.web_origins,
    });
    let url = format!("{base}/admin/realms/{realm}/clients");
    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_json(body);
    match resp {
        Ok(_) => {}
        Err(ureq::Error::Status(409, _)) => {
            // Lost a race with another caller; fall through and refetch.
        }
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            bail!("create client failed (HTTP {code}): {body}");
        }
        Err(e) => bail!("create client request failed: {e}"),
    }

    // 3. Re-lookup the freshly created client to obtain its UUID and secret.
    let uuid = find_client_uuid(base, realm, token, &args.client_id)?
        .ok_or_else(|| anyhow!("client `{}` not found after creation", args.client_id))?;
    let secret = fetch_client_secret(base, realm, token, &uuid)?;
    Ok(ProvisionedClient {
        client_id: args.client_id.clone(),
        client_secret: secret,
        reused: false,
    })
}

fn find_client_uuid(
    base: &str,
    realm: &str,
    token: &str,
    client_id: &str,
) -> Result<Option<String>> {
    let url = format!("{base}/admin/realms/{realm}/clients");
    let resp = ureq::get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .query("clientId", client_id)
        .call();
    let arr: serde_json::Value = match resp {
        Ok(r) => r.into_json().context("clients list response was not JSON")?,
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            bail!("list clients failed (HTTP {code}): {body}");
        }
        Err(e) => bail!("list clients request failed: {e}"),
    };
    let uuid = arr
        .as_array()
        .and_then(|a| a.first())
        .and_then(|c| c.get("id"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(uuid)
}

fn fetch_client_secret(base: &str, realm: &str, token: &str, uuid: &str) -> Result<String> {
    let url = format!("{base}/admin/realms/{realm}/clients/{uuid}/client-secret");
    let resp = ureq::get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .call();
    let body: serde_json::Value = match resp {
        Ok(r) => r
            .into_json()
            .context("client-secret response was not JSON")?,
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            bail!("fetch client secret failed (HTTP {code}): {body}");
        }
        Err(e) => bail!("fetch client secret request failed: {e}"),
    };
    body.get("value")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("client-secret response missing `value`: {body}"))
}
