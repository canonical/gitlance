// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, Subcommand};
use gitlance::{abbreviate_sha, checks, git, output, run_check};
use std::process::exit;

#[derive(Parser)]
#[command(name = "gitlance")]
#[command(about = "Vigilance for your Git commits")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Base git reference (auto-detect from environment if available)
    #[arg(long, global = true)]
    base: Option<String>,

    /// Head git reference (auto-detect from environment if available)
    #[arg(long, global = true)]
    head: Option<String>,

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

/// Displays the list of commits being tested
fn display_commits(commits: &[git::Commit]) {
    let count = commits.len();
    let plural = if count == 1 { "commit" } else { "commits" };
    println!("Testing {} {}:", count, plural);

    for commit in commits {
        let sha_abbrev = abbreviate_sha(&commit.sha);
        let first_line = commit.message.lines().next().unwrap_or("");
        println!("  {} {}", sha_abbrev, first_line);
    }
    println!();
}

fn main() {
    let cli = Cli::parse();

    // Determine which checks to run
    let command = cli.command.unwrap_or(Commands::All);

    // Get refs, with environment variable fallback
    let base = cli.base.or_else(|| std::env::var("BASE_REF").ok());
    let head = cli.head.or_else(|| std::env::var("HEAD_REF").ok());

    // Validate both refs before proceeding
    let mut has_errors = false;

    if base.is_none() {
        output::error(
            "Missing base reference. Provide --base or set BASE_REF environment variable",
        );
        has_errors = true;
    }

    if head.is_none() {
        output::error(
            "Missing head reference. Provide --head or set HEAD_REF environment variable",
        );
        has_errors = true;
    }

    if has_errors {
        exit(1);
    }

    let base = base.expect("base should be validated above");
    let head = head.expect("head should be validated above");

    // Open repository
    let repo = match git::open_repo(&cli.repo) {
        Ok(r) => r,
        Err(e) => {
            output::error(&format!("Failed to open repository: {}", e));
            exit(1);
        }
    };

    // Get commits in range
    let commits = match git::get_commits_in_range(&repo, &base, &head, cli.skip_merge_commits) {
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

    // Display the commits being tested
    display_commits(&commits);

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
