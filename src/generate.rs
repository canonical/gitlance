// SPDX-FileCopyrightText: Canonical Ltd.
//
// SPDX-License-Identifier: Apache-2.0

//! Generates and validates AI-suggested commit messages.
//!
//! Builds a prompt that encodes gitlance's own check rules (conventional
//! commit format, Signed-off-by trailer, no WIP/fixup/squash/amend prefixes),
//! asks an [`LlmProvider`] for a candidate message, and validates the result
//! by running it through the same `check_commits()` functions used to
//! validate real commits. Failing candidates are retried with feedback.

use crate::checks;
use crate::git::Commit;
use std::fmt;

/// Maximum number of attempts (1 initial + retries) before giving up.
const MAX_ATTEMPTS: usize = 3;

#[derive(Debug)]
pub enum GenerateError {
    /// The LLM provider request failed.
    Provider(String),
    /// The provider returned no usable message.
    EmptyResponse,
    /// All attempts produced a message that failed validation.
    ValidationFailed(Vec<(String, String)>),
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerateError::Provider(msg) => write!(f, "LLM provider error: {}", msg),
            GenerateError::EmptyResponse => write!(f, "LLM provider returned an empty response"),
            GenerateError::ValidationFailed(failures) => {
                write!(f, "Generated message failed validation: ")?;
                for (i, (_, reason)) in failures.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{}", reason)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for GenerateError {}

/// Abstraction over an LLM backend, so providers other than OpenRouter can be
/// added later without touching the CLI or validation logic.
pub trait LlmProvider {
    fn generate(&self, prompt: &str) -> Result<String, GenerateError>;
}

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// LLM provider backed by OpenRouter's chat completions API.
pub struct OpenRouterProvider {
    api_key: String,
    model: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

#[derive(serde::Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(serde::Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(serde::Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(serde::Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

impl LlmProvider for OpenRouterProvider {
    fn generate(&self, prompt: &str) -> Result<String, GenerateError> {
        let request = ChatRequest {
            model: &self.model,
            messages: vec![ChatMessage {
                role: "user",
                content: prompt,
            }],
        };

        let response = ureq::post(OPENROUTER_API_URL)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(&request)
            .map_err(|e| GenerateError::Provider(e.to_string()))?;

        let chat_response: ChatResponse = response
            .into_json()
            .map_err(|e| GenerateError::Provider(e.to_string()))?;

        chat_response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content.trim().to_string())
            .filter(|content| !content.is_empty())
            .ok_or(GenerateError::EmptyResponse)
    }
}

/// Builds the prompt sent to the LLM, encoding the exact rules enforced by
/// `checks::conventional`, `checks::wip_fixup`, and `checks::signed_off_by`.
pub fn build_prompt(diff: &str, identity: &(String, String)) -> String {
    let (name, email) = identity;
    format!(
        "You are writing a git commit message for the following staged diff. \
The message MUST satisfy ALL of these rules:\n\
\n\
1. Conventional Commits format: the first line must match \
   `type(scope)?: description`, where type is one of: feat, fix, docs, style, \
   refactor, perf, test, build, ci, chore, revert. A `!` before the colon \
   marks a breaking change (e.g. `feat!: description` or `feat(scope)!: description`).\n\
2. The first line must NOT start with `fixup!`, `squash!`, `amend!`, or `WIP`/`wip` \
   (case-insensitive), and must not be exactly `WIP`.\n\
3. The message must include a trailer line, on its own line, exactly matching: \
   `Signed-off-by: {name} <{email}>`.\n\
\n\
Respond with ONLY the commit message text (first line + blank line + body, \
ending with the Signed-off-by trailer). Do not include any explanation, \
markdown formatting, or code fences.\n\
\n\
Staged diff:\n\
{diff}"
    )
}

/// Wraps a candidate message as a `Commit` and runs it through all three
/// existing checks, returning any failures (empty = valid).
pub fn validate_candidate(message: &str) -> Vec<(String, String)> {
    let commit = Commit {
        sha: String::new(),
        message: message.to_string(),
    };
    let commits = std::slice::from_ref(&commit);

    let mut failures = checks::wip_fixup::check_commits(commits);
    failures.extend(checks::signed_off_by::check_commits(commits));
    failures.extend(checks::conventional::check_commits(commits));
    failures
}

/// Generates a commit message for the given staged diff, validating it
/// against all checks and retrying (with feedback) up to `MAX_ATTEMPTS`
/// times. Never returns a message that fails validation.
pub fn suggest_commit_message(
    provider: &dyn LlmProvider,
    diff: &str,
    identity: &(String, String),
) -> Result<String, GenerateError> {
    let base_prompt = build_prompt(diff, identity);
    let mut prompt = base_prompt.clone();
    let mut last_failures = Vec::new();

    for _ in 0..MAX_ATTEMPTS {
        let candidate = provider.generate(&prompt)?;
        let failures = validate_candidate(&candidate);

        if failures.is_empty() {
            return Ok(candidate);
        }

        let feedback = failures
            .iter()
            .map(|(_, reason)| reason.as_str())
            .collect::<Vec<_>>()
            .join("; ");

        prompt = format!(
            "{base_prompt}\n\n\
The previous attempt failed validation for these reasons: {feedback}. \
Please produce a corrected message that satisfies ALL rules."
        );
        last_failures = failures;
    }

    Err(GenerateError::ValidationFailed(last_failures))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubProvider {
        responses: std::cell::RefCell<Vec<String>>,
    }

    impl LlmProvider for StubProvider {
        fn generate(&self, _prompt: &str) -> Result<String, GenerateError> {
            self.responses
                .borrow_mut()
                .pop()
                .ok_or(GenerateError::EmptyResponse)
        }
    }

    fn identity() -> (String, String) {
        ("Test User".to_string(), "test@example.com".to_string())
    }

    #[test]
    fn test_build_prompt_contains_rules_and_identity() {
        let prompt = build_prompt("diff --git a/f b/f", &identity());
        assert!(prompt.contains("Conventional Commits"));
        assert!(prompt.contains("fixup!"));
        assert!(prompt.contains("Signed-off-by: Test User <test@example.com>"));
        assert!(prompt.contains("diff --git a/f b/f"));
    }

    #[test]
    fn test_validate_candidate_passes_valid_message() {
        let message = "feat: add new feature\n\nSigned-off-by: Test User <test@example.com>";
        assert!(validate_candidate(message).is_empty());
    }

    #[test]
    fn test_validate_candidate_fails_missing_signoff() {
        let failures = validate_candidate("feat: add new feature");
        assert!(!failures.is_empty());
    }

    #[test]
    fn test_validate_candidate_fails_wip() {
        let failures =
            validate_candidate("WIP: add feature\n\nSigned-off-by: Test User <test@example.com>");
        assert!(!failures.is_empty());
    }

    #[test]
    fn test_suggest_commit_message_returns_valid_first_try() {
        let provider = StubProvider {
            responses: std::cell::RefCell::new(vec![
                "feat: add new feature\n\nSigned-off-by: Test User <test@example.com>".to_string(),
            ]),
        };

        let result = suggest_commit_message(&provider, "diff", &identity());
        assert!(result.is_ok());
    }

    #[test]
    fn test_suggest_commit_message_retries_then_succeeds() {
        // Responses are popped in reverse order (Vec::pop takes the last item).
        let provider = StubProvider {
            responses: std::cell::RefCell::new(vec![
                "feat: add new feature\n\nSigned-off-by: Test User <test@example.com>".to_string(),
                "not conventional at all".to_string(),
            ]),
        };

        let result = suggest_commit_message(&provider, "diff", &identity());
        assert!(result.is_ok());
    }

    #[test]
    fn test_suggest_commit_message_fails_after_max_attempts() {
        let provider = StubProvider {
            responses: std::cell::RefCell::new(vec![
                "still not valid".to_string(),
                "still not valid".to_string(),
                "still not valid".to_string(),
            ]),
        };

        let result = suggest_commit_message(&provider, "diff", &identity());
        assert!(matches!(result, Err(GenerateError::ValidationFailed(_))));
    }
}
