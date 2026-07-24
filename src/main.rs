// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, Subcommand};
use gitlance::credential_store::CredentialStoreFactory;
use gitlance::{checks, git, output, run_check};
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

    /// Store your OpenRouter API key for `gitlance suggest` (overwrites any existing key)
    Init,
}

fn main() {
    let mut cli = Cli::parse();
    let command = cli.command.take().unwrap_or(Commands::All);

    match command {
        Commands::Init => run_init(),
        command => run_checks(command, &cli),
    }
}

fn run_init() {
    let store = match CredentialStoreFactory::create_store() {
        Ok(store) => store,
        Err(e) => {
            output::error(&format!("Failed to initialize credential store: {}", e));
            exit(1);
        }
    };

    let overwriting = matches!(store.read_token(), Ok(Some(_)));

    let api_key = match rpassword::prompt_password("OpenRouter API key: ") {
        Ok(key) if !key.trim().is_empty() => key.trim().to_string(),
        Ok(_) => {
            output::error("No API key provided");
            exit(1);
        }
        Err(e) => {
            output::error(&format!("Failed to read API key: {}", e));
            exit(1);
        }
    };

    if let Err(e) = store.write_token(&api_key) {
        output::error(&format!("Failed to store API key: {}", e));
        exit(1);
    }

    let verb = if overwriting { "replaced" } else { "stored" };
    output::notice(&format!(
        "API key {} ({} credential store)",
        verb,
        store.get_name()
    ));
}

fn run_checks(command: Commands, cli: &Cli) {
    // Get refs, with environment variable fallback
    let base = cli.base.clone().or_else(|| std::env::var("BASE_REF").ok());
    let head = cli.head.clone().or_else(|| std::env::var("HEAD_REF").ok());

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
        Commands::Init => {
            unreachable!("handled before run_checks")
        }
    };

    exit(if overall_passed { 0 } else { 1 });
}
