// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, Subcommand};
use gitlance::credential_store::CredentialStoreFactory;
use gitlance::generate::{suggest_commit_message, OpenRouterProvider};
use gitlance::{checks, config::Config, git, output, run_check};
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

    /// Generate a compliant commit message for the currently staged changes
    Suggest {
        /// Commit directly with the validated message instead of printing it
        #[arg(long)]
        commit: bool,
    },
}

fn main() {
    let mut cli = Cli::parse();
    let command = cli.command.take().unwrap_or(Commands::All);

    match command {
        Commands::Init => run_init(),
        Commands::Suggest { commit } => run_suggest(&cli.repo, commit),
        command => run_checks(command, &cli),
    }
}

/// Resolves the OpenRouter API key: `OPENROUTER_API_KEY` env var > stored credential.
fn resolve_api_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY").ok().or_else(|| {
        CredentialStoreFactory::create_store()
            .ok()
            .and_then(|store| store.read_token().ok().flatten())
    })
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

fn run_suggest(repo_path: &str, commit: bool) {
    let api_key = match resolve_api_key() {
        Some(key) => key,
        None => {
            output::error(
                "No OpenRouter API key found. Set OPENROUTER_API_KEY or run `gitlance init`.",
            );
            exit(1);
        }
    };

    let model = Config::load(repo_path).model;

    let repo = match git::open_repo(repo_path) {
        Ok(r) => r,
        Err(e) => {
            output::error(&format!("Failed to open repository: {}", e));
            exit(1);
        }
    };

    let diff = match git::get_staged_diff(&repo) {
        Ok(diff) => diff,
        Err(e) => {
            output::error(&format!("Failed to read staged diff: {}", e));
            exit(1);
        }
    };

    if diff.trim().is_empty() {
        output::error("No staged changes found. Stage changes with `git add` first.");
        exit(1);
    }

    let identity = match git::get_git_identity(&repo) {
        Ok(identity) => identity,
        Err(e) => {
            output::error(&format!("Failed to read git identity: {}", e));
            exit(1);
        }
    };

    let provider = OpenRouterProvider::new(api_key, model);

    let message = match suggest_commit_message(&provider, &diff, &identity) {
        Ok(message) => message,
        Err(e) => {
            output::error(&format!("Failed to generate commit message: {}", e));
            exit(1);
        }
    };

    if commit {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut child = match Command::new("git")
            .current_dir(repo_path)
            .args(["commit", "-F", "-"])
            .stdin(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                output::error(&format!("Failed to run git commit: {}", e));
                exit(1);
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(message.as_bytes()) {
                output::error(&format!("Failed to write commit message: {}", e));
                exit(1);
            }
        }

        match child.wait() {
            Ok(status) if status.success() => println!("{}", message),
            Ok(status) => {
                output::error(&format!(
                    "git commit exited with status {:?}",
                    status.code()
                ));
                exit(1);
            }
            Err(e) => {
                output::error(&format!("Failed to wait for git commit: {}", e));
                exit(1);
            }
        }
    } else {
        println!("{}", message);
    }
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
        Commands::Init | Commands::Suggest { .. } => {
            unreachable!("handled before run_checks")
        }
    };

    exit(if overall_passed { 0 } else { 1 });
}
