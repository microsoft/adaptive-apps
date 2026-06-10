//! Resource-type **catalog** — the set of custom Radius resource types
//! this repository defines in `radius/resource-types/types.yaml`.
//!
//! The catalog is the authoritative list of types the platform actually
//! ships (e.g. `Radius.Resources/postgreSqlDatabases@2025-08-01-preview`).
//! `ada package` consumes it so generation does not guess:
//!
//! * the **compose** analyzer resolves its `type@apiVersion` strings from
//!   the catalog instead of hardcoded constants;
//! * the **llm** / **skill** strategies receive a compact summary of the
//!   available types and are told to use *only* those (overriding any
//!   upstream registry a skill might reference);
//! * a deterministic critic rejects any `Radius.Resources/*` reference
//!   whose type or api-version is not in the catalog.
//!
//! Resolution order (first hit wins):
//! 1. an explicit `--types <path>`;
//! 2. a repo-local `radius/resource-types/types.yaml`, found by walking up
//!    from the input folder and the current directory (dev checkouts);
//! 3. the installed copy at `$ADA_HOME/radius/resource-types/types.yaml`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::home;

/// Relative location of the type definitions inside a repo checkout and
/// inside `$ADA_HOME/radius`.
const TYPES_REL: &str = "resource-types/types.yaml";

/// A parsed resource-type catalog.
#[derive(Debug, Clone)]
pub struct Catalog {
    /// Where the catalog was loaded from (for UI / diagnostics).
    pub source: PathBuf,
    /// Namespace shared by every type, e.g. `Radius.Resources`.
    pub namespace: String,
    pub types: Vec<ResourceType>,
}

#[derive(Debug, Clone)]
pub struct ResourceType {
    /// Short type name, e.g. `postgreSqlDatabases`.
    pub name: String,
    /// Latest api-version declared for the type.
    pub api_version: String,
    /// First non-empty line of the type description.
    pub summary: String,
    /// Writable (non read-only) top-level properties.
    pub properties: Vec<Property>,
    pub required: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub name: String,
    pub ty: String,
    pub enum_values: Vec<String>,
}

impl Catalog {
    /// Resolve and load the catalog. Returns `Ok(None)` when no catalog is
    /// found *and* no explicit path was given (LLM/skill strategies still
    /// work, just without grounding). An explicit `--types` that cannot be
    /// found or parsed is a hard error.
    pub fn load(explicit: Option<&Path>, input: &Path) -> Result<Option<Catalog>> {
        if let Some(path) = explicit {
            let cat = Self::parse_file(path)
                .with_context(|| format!("failed to load --types {}", path.display()))?;
            return Ok(Some(cat));
        }
        if let Some(path) = resolve_path(input) {
            // A discovered path that fails to parse is surfaced rather than
            // silently ignored.
            let cat = Self::parse_file(&path)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            return Ok(Some(cat));
        }
        Ok(None)
    }

    fn parse_file(path: &Path) -> Result<Catalog> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let raw: RawCatalog = serde_yaml::from_str(&text)
            .with_context(|| format!("failed to parse {} as YAML", path.display()))?;

        let mut types = Vec::new();
        for (name, raw_type) in raw.types {
            // Pick the latest api-version. Versions are date-prefixed
            // (`2025-08-01-preview`), so lexicographic max is chronological.
            let Some((api_version, api)) = raw_type
                .api_versions
                .into_iter()
                .max_by(|a, b| a.0.cmp(&b.0))
            else {
                continue;
            };
            let schema = api.schema.unwrap_or_default();
            let properties = schema
                .properties
                .into_iter()
                .filter(|(_, p)| !p.read_only)
                .map(|(pname, p)| Property {
                    name: pname,
                    ty: p.ty.unwrap_or_else(|| "string".into()),
                    enum_values: p
                        .enum_values
                        .into_iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                })
                .collect();
            types.push(ResourceType {
                summary: first_line(&raw_type.description),
                name,
                api_version,
                properties,
                required: schema.required,
            });
        }
        types.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Catalog { source: path.to_path_buf(), namespace: raw.namespace, types })
    }

    /// Fully-qualified `namespace/type@apiVersion` for a short type name.
    pub fn type_ref(&self, type_name: &str) -> Option<String> {
        self.types
            .iter()
            .find(|t| t.name == type_name)
            .map(|t| format!("{}/{}@{}", self.namespace, t.name, t.api_version))
    }

    /// Whether the catalog declares a type at the given api-version.
    pub fn has(&self, type_name: &str, api_version: &str) -> bool {
        self.types
            .iter()
            .any(|t| t.name == type_name && t.api_version == api_version)
    }

    /// Whether the catalog declares a type at any api-version.
    pub fn has_type(&self, type_name: &str) -> bool {
        self.types.iter().any(|t| t.name == type_name)
    }

    /// Compact, prompt-friendly listing of every type with its
    /// api-version and writable properties.
    pub fn render_summary(&self) -> String {
        let mut out = format!(
            "Platform resource types defined by this repository (namespace `{}`). \
These are the ONLY `{}` types available — do not invent others or reference \
external type registries:\n",
            self.namespace, self.namespace
        );
        for t in &self.types {
            out.push_str(&format!(
                "\n- {}/{}@{}\n  {}\n",
                self.namespace, t.name, t.api_version, t.summary
            ));
            if !t.properties.is_empty() {
                out.push_str("  writable properties: ");
                let props: Vec<String> = t
                    .properties
                    .iter()
                    .map(|p| {
                        let req = if t.required.contains(&p.name) { "*" } else { "" };
                        if p.enum_values.is_empty() {
                            format!("{}{} ({})", p.name, req, p.ty)
                        } else {
                            format!("{}{} ({})", p.name, req, p.enum_values.join("|"))
                        }
                    })
                    .collect();
                out.push_str(&props.join(", "));
                out.push('\n');
            }
        }
        out.push_str("\n(* = required; readOnly/output properties are omitted.)\n");
        out
    }
}

/// Scan generated bicep for `<Catalog.namespace>/<type>@<version>`
/// references. Returns `(type_name, api_version)` pairs (best-effort
/// textual scan; no full bicep parser needed).
pub fn referenced_types(bicep: &str, namespace: &str) -> Vec<(String, String)> {
    let needle = format!("{namespace}/");
    let mut out = Vec::new();
    let bytes = bicep.as_bytes();
    let mut search_from = 0;
    while let Some(rel) = bicep[search_from..].find(&needle) {
        let start = search_from + rel + needle.len();
        // type name: identifier chars.
        let mut i = start;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        let type_name = &bicep[start..i];
        // optional @version
        let mut version = String::new();
        if i < bytes.len() && bytes[i] == b'@' {
            let vstart = i + 1;
            let mut j = vstart;
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-' || bytes[j] == b'.')
            {
                j += 1;
            }
            version = bicep[vstart..j].to_string();
            i = j;
        }
        if !type_name.is_empty() {
            out.push((type_name.to_string(), version));
        }
        search_from = i.max(start + 1);
    }
    out
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

fn resolve_path(input: &Path) -> Option<PathBuf> {
    // 1. Walk up from the input folder, then the current dir, looking for a
    //    repo-local `radius/resource-types/types.yaml`.
    for start in [input.to_path_buf(), std::env::current_dir().ok()?] {
        if let Some(p) = find_repo_types(&start) {
            return Some(p);
        }
    }
    // 2. Fall back to the installed copy under $ADA_HOME/radius.
    if let Ok(radius_root) = home::radius_root() {
        let installed = radius_root.join(TYPES_REL);
        if installed.is_file() {
            return Some(installed);
        }
    }
    None
}

fn find_repo_types(start: &Path) -> Option<PathBuf> {
    let mut dir = start.canonicalize().ok()?;
    loop {
        let candidate = dir.join("radius").join(TYPES_REL);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn first_line(s: &str) -> String {
    s.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

// ---------------------------------------------------------------------------
// Raw deserialization shapes (match types.yaml layout)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawCatalog {
    namespace: String,
    #[serde(default)]
    types: BTreeMap<String, RawType>,
}

#[derive(Debug, Deserialize)]
struct RawType {
    #[serde(default)]
    description: String,
    #[serde(default, rename = "apiVersions")]
    api_versions: BTreeMap<String, RawApiVersion>,
}

#[derive(Debug, Deserialize)]
struct RawApiVersion {
    #[serde(default)]
    schema: Option<RawSchema>,
}

#[derive(Debug, Default, Deserialize)]
struct RawSchema {
    #[serde(default)]
    properties: BTreeMap<String, RawProp>,
    #[serde(default)]
    required: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawProp {
    #[serde(default, rename = "type")]
    ty: Option<String>,
    #[serde(default, rename = "readOnly")]
    read_only: bool,
    #[serde(default, rename = "enum")]
    enum_values: Vec<serde_yaml::Value>,
}

/// Convenience for callers that want a hard failure when nothing resolves.
#[allow(dead_code)]
pub fn require(explicit: Option<&Path>, input: &Path) -> Result<Catalog> {
    Catalog::load(explicit, input)?.ok_or_else(|| {
        anyhow!(
            "no resource-type catalog found; pass --types or run `ada init` to install \
             $ADA_HOME/radius/{TYPES_REL}"
        )
    })
}
