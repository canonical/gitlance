// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, Subcommand};
use gitlance::{checks, git, output, run_check};
use std::process::exit;

#[derive(Parser)]
#[command(name = "gitlance")]
#[command(about = "Vigilance for your Git commits")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Base commit SHA (auto-detect from environment if available)
    #[arg(long, global = true)]
    base_sha: Option<String>,

    /// Head commit SHA (auto-detect from environment if available)
    #[arg(long, global = true)]
    head_sha: Option<String>,

    /// Git repository path
    #[arg(long, global = true, default_value = ".")]
    repo: String,

    /// Skip merge commits in validation (default: false, all commits are checked)
    #[arg(long, global = true)]
    skip_merge_commits: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Check for WIP/fixup/squash/amend commits
    WipFixup,

    /// Check for Signed-off-by trailers
    SignedOffBy,

    /// Check conventional commit format
    ConventionalCommits,

    /// Run all checks (default if no command given)
    All,
}

fn main() {
    let cli = Cli::parse();

    // Determine which checks to run
    let command = cli.command.unwrap_or(Commands::All);

    // Get SHAs, with environment variable fallback
    let base_sha = cli.base_sha.or_else(|| std::env::var("BASE_SHA").ok());
    let head_sha = cli.head_sha.or_else(|| std::env::var("HEAD_SHA").ok());

    // Validate both SHAs before proceeding
    let mut has_errors = false;

    if base_sha.is_none() {
        output::error("Missing base SHA. Provide --base-sha or set BASE_SHA environment variable");
        has_errors = true;
    }

    if head_sha.is_none() {
        output::error("Missing head SHA. Provide --head-sha or set HEAD_SHA environment variable");
        has_errors = true;
    }

    if has_errors {
        exit(1);
    }

    let base_sha = base_sha.expect("base_sha should be validated above");
    let head_sha = head_sha.expect("head_sha should be validated above");

    // Open repository
    let repo = match git::open_repo(&cli.repo) {
        Ok(r) => r,
        Err(e) => {
            output::error(&format!("Failed to open repository: {}", e));
            exit(1);
        }
    };

    // Get commits in range
    let commits =
        match git::get_commits_in_range(&repo, &base_sha, &head_sha, cli.skip_merge_commits) {
            Ok(commits) => commits,
            Err(e) => {
                output::error(&format!("Failed to get commits: {}", e));
                exit(1);
            }
        };

    if commits.is_empty() {
        output::notice("No commits found in the specified range");
        exit(0);
    }

    // Run the appropriate check(s)
    let overall_passed = match command {
        Commands::WipFixup => run_check("WIP/Fixup", &checks::wip_fixup::check_commits(&commits)),
        Commands::SignedOffBy => run_check(
            "Signed-off-by",
            &checks::signed_off_by::check_commits(&commits),
        ),
        Commands::ConventionalCommits => run_check(
            "Conventional Commits",
            &checks::conventional::check_commits(&commits),
        ),
        Commands::All => {
            let wip_failures = checks::wip_fixup::check_commits(&commits);
            let signoff_failures = checks::signed_off_by::check_commits(&commits);
            let conventional_failures = checks::conventional::check_commits(&commits);

            run_check("WIP/Fixup", &wip_failures);
            run_check("Signed-off-by", &signoff_failures);
            run_check("Conventional Commits", &conventional_failures);

            wip_failures.is_empty()
                && signoff_failures.is_empty()
                && conventional_failures.is_empty()
        }
    };

    exit(if overall_passed { 0 } else { 1 });
}
