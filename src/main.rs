//! Guardener runs the suiflex engineering standard across the organization's
//! repositories.
//!
//! It is a plain binary rather than a service. Every subcommand is a single
//! pass that reads the world, decides, writes once, and exits, so the same code
//! runs from a workflow today and behind a webhook later without being rewritten.

mod check;
mod config;
mod github;
mod hygiene;
mod review;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use github::Client;

#[derive(Parser)]
#[command(name = "guardener", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the ForgeGuard gate over a pull request's changed lines.
    Check {
        /// The checkout to analyse.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// The watched-repository registry.
        #[arg(long, default_value = "config/repositories.toml")]
        registry: PathBuf,
        /// The ForgeGuard policy used by repositories that ship none of their own.
        #[arg(long, default_value = "config/forgeguard.toml")]
        organization_default: PathBuf,
        /// The repository as owner/name.
        #[arg(long)]
        repo: String,
        /// The pull request number.
        #[arg(long)]
        pr: u64,
        /// The commit the check run is reported against.
        #[arg(long)]
        head_sha: String,
        /// The revision the pull request is measured against.
        #[arg(long, default_value = "origin/main")]
        base: String,
        /// Skip the configured quality commands. Required for branches from
        /// outside the organization, whose configuration must not be executed.
        #[arg(long)]
        untrusted: bool,
        /// Print what would be written to GitHub, and write nothing.
        #[arg(long)]
        dry_run: bool,
    },

    /// Check every watched repository still carries the organization standard.
    Hygiene {
        /// The watched-repository registry.
        #[arg(long, default_value = "config/repositories.toml")]
        registry: PathBuf,
        /// The labels the organization's triage workflow needs to exist.
        #[arg(long, default_value = "config/labels.toml")]
        labels: PathBuf,
        /// Where the standing report is kept, as owner/name.
        #[arg(long, default_value = "suiflex/Guardener")]
        report_to: String,
        /// Target a single repository instead of the whole registry.
        #[arg(long)]
        repo: Option<String>,
        /// Open a pull request adding the standard files a repository is
        /// missing. Never edits or replaces a file that already exists, and
        /// never changes a setting.
        #[arg(long)]
        fix: bool,
        /// Print what would be written to GitHub, and write nothing.
        #[arg(long)]
        dry_run: bool,
    },

    /// Have a model read a pull request and say what the scanner cannot.
    ///
    /// Names a pull request with --repo and --pr. Without --pr it sweeps
    /// instead, for open pull requests that sat still and were never reviewed:
    /// every watched repository, or just the one --repo names.
    Review {
        /// The watched-repository registry.
        #[arg(long, default_value = "config/repositories.toml")]
        registry: PathBuf,
        /// How much of a change is worth reading, and what to leave out.
        #[arg(long, default_value = "config/review.toml")]
        settings: PathBuf,
        /// The repository as owner/name. With --pr, the one to review; without
        /// it, the only one to sweep. Omit both to sweep the whole registry.
        #[arg(long)]
        repo: Option<String>,
        /// The pull request number. Omit to sweep the registry instead.
        #[arg(long)]
        pr: Option<u64>,
        /// How long a pull request must have sat untouched before a sweep will
        /// look at it. Zero means age is no bar, which is the right default now
        /// that the sweep is the only thing reviewing on its own: a pull request
        /// opened this morning still deserves reading. Ignored when --pr names
        /// one.
        #[arg(long, default_value_t = 0)]
        stale_days: u64,
        /// The most pull requests one sweep will review. High enough that a
        /// normal week fits inside it, finite because the first sweep meets
        /// every pull request that was never reviewed at once, and a bill nobody
        /// asked for is a poor way to find that out.
        #[arg(long, default_value_t = 30)]
        max: usize,
        /// Print what would be written to GitHub, and write nothing. Reviewing
        /// one pull request still asks the model: what it says is the thing
        /// worth previewing. A sweep does not — there the preview is which
        /// pull requests would be read, and asking would cost the whole run.
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check {
            root,
            registry,
            organization_default,
            repo,
            pr,
            head_sha,
            base,
            untrusted,
            dry_run,
        } => {
            let client = Client::new(token()?, dry_run);
            check::run(
                &client,
                &check::Request {
                    root: &root,
                    registry: &registry,
                    organization_default: &organization_default,
                    repo: &repo,
                    pull_request: pr,
                    head_sha: &head_sha,
                    base: &base,
                    untrusted,
                },
            )
        }
        Command::Review {
            registry,
            settings,
            repo,
            pr,
            stale_days,
            max,
            dry_run,
        } => {
            let client = Client::new(token()?, dry_run);
            review::run(
                &client,
                &review::Request {
                    registry: &registry,
                    settings: &settings,
                    repo: repo.as_deref(),
                    pull_request: pr,
                    stale_days,
                    max,
                    endpoint: &required("GUARDENER_MODEL_URL")?,
                    key: &required("GUARDENER_MODEL_KEY")?,
                    model: &required("GUARDENER_MODEL")?,
                },
            )
        }
        Command::Hygiene {
            registry,
            labels,
            report_to,
            repo,
            fix,
            dry_run,
        } => {
            let client = Client::new(token()?, dry_run);
            hygiene::run(
                &client,
                &hygiene::Request {
                    registry: &registry,
                    labels: &labels,
                    report_to: &report_to,
                    repo: repo.as_deref(),
                    fix,
                },
            )
        }
    }
}

/// The credential every run needs, dry or not.
///
/// A dry run withholds the writes, not the reads: it has to know whether the
/// comment it would edit already exists, and what a repository already
/// contains, or the preview it prints is a guess. So it is read-only, not
/// offline, and the token is required either way. A read-only personal token is
/// enough to try one out by hand.
/// Named rather than defaulted. A missing endpoint should stop the run and say
/// which variable is missing, not quietly review nothing or, worse, send a
/// private diff somewhere nobody chose.
fn required(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{name} is not set"))
}

fn token() -> Result<String> {
    std::env::var("GUARDENER_TOKEN")
        .ok()
        .filter(|token| !token.is_empty())
        .context(
            "GUARDENER_TOKEN is not set; the workflow supplies it from the suiflex-bot app token, \
         and a dry run needs one too because it still reads",
        )
}
