//! The organization registry, and how a repository's ForgeGuard policy is decided.
//!
//! ForgeGuard is a local tool: it expects every repository to carry its own
//! `.forgeguard/config.toml` and refuses to run without one. Most repositories
//! in the organization do not have that file, and should not each keep a copy
//! of a policy that is meant to be the same everywhere. So the policy lives
//! here once, in `config/forgeguard.toml`, and a repository only writes its own
//! file when it genuinely needs to differ.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use forgeguard_core::config::{GuardMode, RuleConfig};
use forgeguard_core::ForgeGuardConfig;
use serde::Deserialize;

/// Where ForgeGuard keeps a repository's own configuration, relative to the
/// repository root. Spelled out rather than imported because the constant is
/// private to forgeguard-core.
const REPO_CONFIG: &str = ".forgeguard/config.toml";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Registry {
    #[serde(default, rename = "repository")]
    pub repositories: Vec<RepositoryEntry>,
}

/// One watched repository. Everything past `name` is an exception to the
/// organization default; an entry that needs no exception is one line.
#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryEntry {
    /// `owner/name`, exactly as GitHub spells it.
    pub name: String,
    #[serde(default)]
    pub mode: Option<GuardMode>,
    #[serde(default)]
    pub rules: BTreeMap<String, RuleConfig>,
    /// Hygiene checks this repository is excused from, by check name. An
    /// exception lives here so it is visible next to every other repository,
    /// rather than being inferred from the absence of a complaint.
    #[serde(default)]
    pub exempt: Vec<String>,
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&source).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn find(&self, repo: &str) -> Option<&RepositoryEntry> {
        self.repositories
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(repo))
    }
}

/// Decides which ForgeGuard configuration governs a checkout.
///
/// A repository that ships its own file has thought about this and wins
/// outright — the organization default is a floor, not a ceiling. Registry
/// overrides apply either way, so an exception can be granted centrally
/// without a pull request against the repository itself.
pub fn resolve(
    root: &Path,
    organization_default: &Path,
    repo: &str,
    entry: Option<&RepositoryEntry>,
) -> Result<ForgeGuardConfig> {
    let mut config = if root.join(REPO_CONFIG).is_file() {
        ForgeGuardConfig::load(root)?
    } else {
        let source = std::fs::read_to_string(organization_default).with_context(|| {
            format!(
                "failed to read organization default {}",
                organization_default.display()
            )
        })?;
        let mut config: ForgeGuardConfig = toml::from_str(&source).with_context(|| {
            format!(
                "failed to parse organization default {}",
                organization_default.display()
            )
        })?;
        config.project.name = short_name(repo).to_string();
        config
    };

    if let Some(entry) = entry {
        if let Some(mode) = entry.mode {
            config.mode = mode;
        }
        for (rule_id, rule) in &entry.rules {
            config.rules.insert(rule_id.clone(), rule.clone());
        }
    }

    Ok(config)
}

fn short_name(repo: &str) -> &str {
    repo.rsplit('/').next().unwrap_or(repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgeguard_core::model::Severity;

    fn organization_default(directory: &Path) -> std::path::PathBuf {
        let path = directory.join("forgeguard.toml");
        std::fs::write(
            &path,
            "version = 2\nmode = \"default\"\n\n[project]\nname = \"placeholder\"\n",
        )
        .unwrap();
        path
    }

    #[test]
    fn falls_back_to_the_organization_default_and_names_the_project() {
        let temporary = tempfile::tempdir().unwrap();
        let default = organization_default(temporary.path());
        let root = temporary.path().join("checkout");
        std::fs::create_dir_all(&root).unwrap();

        let config = resolve(&root, &default, "suiflex/rdb", None).unwrap();

        assert_eq!(config.project.name, "rdb");
        assert_eq!(config.mode, GuardMode::Default);
    }

    #[test]
    fn a_repository_that_ships_its_own_configuration_wins() {
        let temporary = tempfile::tempdir().unwrap();
        let default = organization_default(temporary.path());
        let root = temporary.path().join("checkout");
        std::fs::create_dir_all(root.join(".forgeguard")).unwrap();
        std::fs::write(
            root.join(REPO_CONFIG),
            "version = 2\nmode = \"strict\"\n\n[project]\nname = \"chosen-by-the-repository\"\n",
        )
        .unwrap();

        let config = resolve(&root, &default, "suiflex/rdb", None).unwrap();

        assert_eq!(config.project.name, "chosen-by-the-repository");
        assert_eq!(config.mode, GuardMode::Strict);
    }

    #[test]
    fn registry_overrides_apply_on_top_of_either_source() {
        let temporary = tempfile::tempdir().unwrap();
        let default = organization_default(temporary.path());
        let root = temporary.path().join("checkout");
        std::fs::create_dir_all(&root).unwrap();

        let registry: Registry = toml::from_str(
            r#"
            [[repository]]
            name = "suiflex/rdb"
            mode = "strict"
            rules = { "FG-DRY-003" = { enabled = false }, "FG-ALG-001" = { severity = "error" } }
            "#,
        )
        .unwrap();
        let entry = registry.find("suiflex/rdb").expect("entry");

        let config = resolve(&root, &default, "suiflex/rdb", Some(entry)).unwrap();

        assert_eq!(config.mode, GuardMode::Strict);
        assert_eq!(config.rules["FG-DRY-003"].enabled, Some(false));
        assert_eq!(config.rules["FG-ALG-001"].severity, Some(Severity::Error));
    }

    /// Guards the shipped policy, not the resolver. `strict` is a word in a
    /// file; what matters is that a plain warning actually blocks under it,
    /// which a stray `warnings_block = false` would silently undo.
    #[test]
    fn the_shipped_policy_blocks_on_warnings() {
        let temporary = tempfile::tempdir().unwrap();
        let config = resolve(
            temporary.path(),
            Path::new("config/forgeguard.toml"),
            "suiflex/rdb",
            None,
        )
        .unwrap();

        assert_eq!(config.mode, GuardMode::Strict);
        assert!(config.blocks_finding("FG-ALG-001", Severity::Warning));
        assert!(config.blocks_finding("FG-SEC-001", Severity::Error));
    }

    /// A typo here breaks the daily sweep silently, and nothing else would
    /// catch it until someone noticed the report had stopped arriving.
    #[test]
    fn the_shipped_registry_parses() {
        let registry = Registry::load(Path::new("config/repositories.toml")).unwrap();

        assert!(!registry.repositories.is_empty());
        for entry in &registry.repositories {
            assert!(
                entry.name.starts_with("suiflex/"),
                "{} is not in the organization",
                entry.name
            );
        }
        assert!(registry.find("suiflex/homebrew-tap").is_none());
        assert!(registry.find("suiflex/scoop-bucket").is_none());
    }

    #[test]
    fn lookup_is_case_insensitive_because_github_is() {
        let registry: Registry =
            toml::from_str("[[repository]]\nname = \"suiflex/ForgeGuard\"\n").unwrap();

        assert!(registry.find("suiflex/forgeguard").is_some());
        assert!(registry.find("suiflex/websift").is_none());
    }
}
