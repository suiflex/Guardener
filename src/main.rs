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
        /// Open a pull request adding the standard files a repository is
        /// missing. Never edits or replaces a file that already exists, and
        /// never changes a setting.
        #[arg(long)]
        fix: bool,
        /// Print what would be written to GitHub, and write nothing.
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
        Command::Hygiene {
            registry,
            labels,
            report_to,
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
fn token() -> Result<String> {
    std::env::var("GUARDENER_TOKEN")
        .ok()
        .filter(|token| !token.is_empty())
        .context(
            "GUARDENER_TOKEN is not set; the workflow supplies it from the suiflex-bot app token, \
         and a dry run needs one too because it still reads",
        )
}
