//! `ada skill` — manage external agent-skill packs that `ada package`
//! can use as a generation strategy (e.g. the upstream
//! [`radius-project/radius-skills`](https://github.com/radius-project/radius-skills)
//! `app-modeling` skill).
//!
//! Skills live in their own GitHub repositories and evolve independently
//! of this CLI. To stay current *without* sacrificing reproducibility we
//! follow a fetch + cache + lockfile model (the same shape as
//! `npx skills add <owner/repo>`):
//!
//! ```text
//! $ADA_HOME/skills/
//!   skills.lock                       # resolved commit SHA per repo
//!   <owner>/<repo>/<skill-name>/
//!     SKILL.md
//!     references/*.md
//! ```
//!
//! * `ada skill add <owner/repo> [--ref <branch|tag|sha>]` resolves the
//!   ref to a concrete commit SHA, downloads every file under the repo's
//!   `skills/` tree pinned to that SHA, and records the SHA in
//!   `skills.lock`.
//! * `ada skill update [<owner/repo>]` re-resolves the recorded ref and
//!   rewrites the lock if upstream moved.
//! * `ada skill list` prints what is cached and its pinned SHA.
//!
//! `ada package --strategy skill` reads the cached pack offline and pins
//! generation to the locked SHA, so the same input always produces the
//! same prompt.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::home;
use crate::ui;

const GITHUB_API: &str = "https://api.github.com";
const RAW_BASE: &str = "https://raw.githubusercontent.com";
const USER_AGENT: &str = concat!("ada-cli/", env!("CARGO_PKG_VERSION"));

/// Default skill repository used by `ada package --strategy skill` and as
/// the positional default for `ada skill add`.
pub const DEFAULT_SKILL_REPO: &str = "radius-project/radius-skills";
/// Default skill within [`DEFAULT_SKILL_REPO`].
pub const DEFAULT_SKILL_NAME: &str = "app-modeling";

const LOCK_VERSION: u32 = 1;

#[derive(Debug, Args)]
pub struct SkillArgs {
    #[command(subcommand)]
    command: SkillCommand,
}

#[derive(Debug, Subcommand)]
enum SkillCommand {
    /// Download a skill repo's `skills/` tree into the local cache and
    /// pin the resolved commit SHA in `skills.lock`.
    Add(AddArgs),

    /// Re-resolve cached skill repos against their recorded ref and
    /// refresh the cache + lock when upstream has moved.
    Update(UpdateArgs),

    /// List cached skill repos and their pinned commit SHAs.
    List,
}

#[derive(Debug, Args)]
struct AddArgs {
    /// Skill repository in `owner/repo` form.
    #[arg(default_value = DEFAULT_SKILL_REPO)]
    repo: String,

    /// Branch, tag, or commit SHA to pin. Defaults to the repo's default
    /// branch head at fetch time.
    #[arg(long, default_value = "main")]
    r#ref: String,
}

#[derive(Debug, Args)]
struct UpdateArgs {
    /// Repository to update (`owner/repo`). When omitted, every cached
    /// repo is updated.
    repo: Option<String>,
}

// ---------------------------------------------------------------------------
// Command entry points
// ---------------------------------------------------------------------------

pub fn run(args: SkillArgs) -> Result<()> {
    match args.command {
        SkillCommand::Add(a) => run_add(a),
        SkillCommand::Update(a) => run_update(a),
        SkillCommand::List => run_list(),
    }
}

fn run_add(args: AddArgs) -> Result<()> {
    let repo = RepoRef::parse(&args.repo)?;
    ui::heading("skill add");
    ui::detail("repo", &repo.slug());
    ui::detail("ref", &args.r#ref);

    let entry = fetch_repo(&repo, &args.r#ref)?;
    let mut lock = load_lock()?;
    lock.skills.insert(repo.slug(), entry.clone());
    save_lock(&lock)?;

    ui::ok(&format!(
        "cached {} files at {} (sha {})",
        entry.files,
        repo.slug(),
        short_sha(&entry.sha)
    ));
    Ok(())
}

fn run_update(args: UpdateArgs) -> Result<()> {
    let mut lock = load_lock()?;
    if lock.skills.is_empty() {
        ui::note("no skills cached; run `ada skill add <owner/repo>` first");
        return Ok(());
    }

    let targets: Vec<String> = match args.repo {
        Some(slug) => {
            let repo = RepoRef::parse(&slug)?;
            if !lock.skills.contains_key(&repo.slug()) {
                bail!("{} is not cached; run `ada skill add {}` first", repo.slug(), repo.slug());
            }
            vec![repo.slug()]
        }
        None => lock.skills.keys().cloned().collect(),
    };

    ui::heading("skill update");
    for slug in targets {
        let repo = RepoRef::parse(&slug)?;
        let prev = lock.skills.get(&slug).cloned().unwrap_or_default();
        ui::step(&format!("{} (ref {})", slug, prev.r#ref));
        let entry = fetch_repo(&repo, &prev.r#ref)?;
        if entry.sha == prev.sha {
            ui::ok(&format!("up to date ({})", short_sha(&entry.sha)));
        } else {
            ui::ok(&format!(
                "updated {} -> {}",
                short_sha(&prev.sha),
                short_sha(&entry.sha)
            ));
        }
        lock.skills.insert(slug, entry);
    }
    save_lock(&lock)?;
    Ok(())
}

fn run_list() -> Result<()> {
    let lock = load_lock()?;
    ui::heading("cached skills");
    ui::detail("cache", &skills_root()?.display().to_string());
    if lock.skills.is_empty() {
        ui::bullet("(none) — run `ada skill add <owner/repo>`");
        return Ok(());
    }
    for (slug, entry) in &lock.skills {
        ui::bullet(&format!(
            "{slug}  ref={}  sha={}  files={}",
            entry.r#ref,
            short_sha(&entry.sha),
            entry.files
        ));
        for name in cached_skill_names(slug) {
            ui::bullet(&format!("    - {name}"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public loader used by `ada package --strategy skill`
// ---------------------------------------------------------------------------

/// Read a cached skill into a single instruction string: the `SKILL.md`
/// body followed by every reference file under `references/`. Callers
/// wrap this with their own framing (e.g. neutralizing the skill's chat
/// "Output Format" section) before sending it to a model.
///
/// `repo` is `owner/repo`; `skill_name` is the directory under the repo's
/// `skills/` tree (e.g. `app-modeling`).
pub fn load_pack(repo: &str, skill_name: &str) -> Result<SkillPack> {
    let repo_ref = RepoRef::parse(repo)?;
    let dir = skills_root()?
        .join(&repo_ref.owner)
        .join(&repo_ref.repo)
        .join(skill_name);
    let skill_md = dir.join("SKILL.md");
    if !skill_md.is_file() {
        bail!(
            "skill `{skill_name}` from {repo} is not cached at {}; run `ada skill add {repo}` first",
            dir.display()
        );
    }

    let mut text = fs::read_to_string(&skill_md)
        .with_context(|| format!("failed to read {}", skill_md.display()))?;

    // Append reference files (architecture patterns, naming rules, …) so
    // the model has the full ruleset in one prompt.
    let refs_dir = dir.join("references");
    if refs_dir.is_dir() {
        let mut paths: Vec<PathBuf> = fs::read_dir(&refs_dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
                    .collect()
            })
            .unwrap_or_default();
        paths.sort();
        for p in paths {
            if let Ok(body) = fs::read_to_string(&p) {
                let name = p
                    .strip_prefix(&dir)
                    .unwrap_or(&p)
                    .display()
                    .to_string();
                text.push_str(&format!("\n\n=== {name} ===\n{body}"));
            }
        }
    }

    let sha = load_lock()
        .ok()
        .and_then(|l| l.skills.get(&repo_ref.slug()).map(|e| e.sha.clone()))
        .unwrap_or_default();

    Ok(SkillPack {
        repo: repo_ref.slug(),
        skill: skill_name.to_string(),
        sha,
        instructions: text,
    })
}

/// A loaded skill ready to feed into a generation prompt.
#[derive(Debug, Clone)]
pub struct SkillPack {
    pub repo: String,
    pub skill: String,
    /// Pinned commit SHA the cache was fetched at (empty if unknown).
    pub sha: String,
    /// Concatenated SKILL.md + references body.
    pub instructions: String,
}

// ---------------------------------------------------------------------------
// Fetch + cache
// ---------------------------------------------------------------------------

fn fetch_repo(repo: &RepoRef, gitref: &str) -> Result<LockEntry> {
    ui::step("resolving ref");
    let sha = resolve_sha(repo, gitref)?;
    ui::step(&format!("listing skills/ tree @ {}", short_sha(&sha)));
    let files = list_skill_files(repo, &sha)?;

    let dest = skills_root()?.join(&repo.owner).join(&repo.repo);
    // Replace any prior cache for this repo so removed upstream files do
    // not linger.
    if dest.exists() {
        fs::remove_dir_all(&dest)
            .with_context(|| format!("failed to clear cache dir {}", dest.display()))?;
    }

    ui::step(&format!("downloading {} files", files.len()));
    for path in &files {
        let bytes = download_raw(repo, &sha, path)?;
        // Strip the leading `skills/` so the cache layout is
        // `<owner>/<repo>/<skill-name>/...`.
        let rel = path.strip_prefix("skills/").unwrap_or(path);
        let out = dest.join(rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&out, &bytes)
            .with_context(|| format!("failed to write {}", out.display()))?;
    }

    Ok(LockEntry {
        r#ref: gitref.to_string(),
        sha,
        fetched_at: now_unix(),
        files: files.len(),
    })
}

fn resolve_sha(repo: &RepoRef, gitref: &str) -> Result<String> {
    let url = format!("{GITHUB_API}/repos/{}/{}/commits/{gitref}", repo.owner, repo.repo);
    let v: serde_json::Value = github_get(&url)?
        .into_json()
        .context("GitHub commits response was not valid JSON")?;
    v.get("sha")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("could not resolve ref `{gitref}` for {}", repo.slug()))
}

fn list_skill_files(repo: &RepoRef, sha: &str) -> Result<Vec<String>> {
    let url = format!(
        "{GITHUB_API}/repos/{}/{}/git/trees/{sha}?recursive=1",
        repo.owner, repo.repo
    );
    let v: serde_json::Value = github_get(&url)?
        .into_json()
        .context("GitHub trees response was not valid JSON")?;
    let tree = v
        .get("tree")
        .and_then(|t| t.as_array())
        .ok_or_else(|| anyhow!("GitHub trees response missing `tree` array"))?;

    let mut files = Vec::new();
    for entry in tree {
        if entry.get("type").and_then(|t| t.as_str()) != Some("blob") {
            continue;
        }
        if let Some(path) = entry.get("path").and_then(|p| p.as_str()) {
            if path.starts_with("skills/") {
                files.push(path.to_string());
            }
        }
    }
    if files.is_empty() {
        bail!("{} has no files under a `skills/` directory", repo.slug());
    }
    Ok(files)
}

fn download_raw(repo: &RepoRef, sha: &str, path: &str) -> Result<Vec<u8>> {
    let url = format!("{RAW_BASE}/{}/{}/{sha}/{path}", repo.owner, repo.repo);
    let resp = match ureq::get(&url).set("User-Agent", USER_AGENT).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => bail!("download {url} returned HTTP {code}"),
        Err(e) => bail!("download {url} failed: {e}"),
    };
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .with_context(|| format!("failed reading body for {url}"))?;
    Ok(buf)
}

fn github_get(url: &str) -> Result<ureq::Response> {
    let mut req = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.trim().is_empty() {
            req = req.set("Authorization", &format!("Bearer {}", token.trim()));
        }
    }
    match req.call() {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            let hint = if code == 403 {
                " (GitHub rate limit? set GITHUB_TOKEN to raise it)"
            } else {
                ""
            };
            bail!("GitHub API {url} returned HTTP {code}{hint}: {body}")
        }
        Err(e) => bail!("GitHub request to {url} failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Lockfile
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct Lock {
    version: u32,
    #[serde(default)]
    skills: BTreeMap<String, LockEntry>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct LockEntry {
    r#ref: String,
    sha: String,
    fetched_at: u64,
    files: usize,
}

fn lock_path() -> Result<PathBuf> {
    Ok(skills_root()?.join("skills.lock"))
}

fn load_lock() -> Result<Lock> {
    let path = lock_path()?;
    if !path.is_file() {
        return Ok(Lock { version: LOCK_VERSION, skills: BTreeMap::new() });
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn save_lock(lock: &Lock) -> Result<()> {
    let path = lock_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(lock).context("failed to serialize skills.lock")?;
    fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// `$ADA_HOME/skills`.
pub fn skills_root() -> Result<PathBuf> {
    Ok(home::root()?.join("skills"))
}

/// Skill directory names cached for a repo slug (best-effort, for display).
fn cached_skill_names(slug: &str) -> Vec<String> {
    let Ok(repo) = RepoRef::parse(slug) else { return Vec::new() };
    let Ok(root) = skills_root() else { return Vec::new() };
    let dir = root.join(&repo.owner).join(&repo.repo);
    let mut names = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for entry in rd.flatten() {
            if entry.path().is_dir() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    names
}

struct RepoRef {
    owner: String,
    repo: String,
}

impl RepoRef {
    fn parse(slug: &str) -> Result<Self> {
        let slug = slug.trim().trim_end_matches('/');
        let (owner, repo) = slug
            .split_once('/')
            .ok_or_else(|| anyhow!("expected `owner/repo`, got `{slug}`"))?;
        if owner.is_empty() || repo.is_empty() || repo.contains('/') {
            bail!("expected `owner/repo`, got `{slug}`");
        }
        Ok(Self { owner: owner.to_string(), repo: repo.to_string() })
    }

    fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
