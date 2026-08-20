<!--
SPDX-FileCopyrightText: Canonical Ltd.

SPDX-License-Identifier: Apache-2.0
-->

# Contributing to gitlance

Contributions are welcome! Please follow these guidelines.

## Development Workflow

1. Fork and create a feature branch from `main`
2. Make your changes
3. Run `task validate` to ensure all checks pass
4. Commit using [conventional commit format](https://www.conventionalcommits.org/)
5. Add `Signed-off-by` trailer to commits
6. Open a pull request

### Local commit validation

Gitlance can validate commit messages and pushed commits locally. Install
[pre-commit](https://pre-commit.com/#install), install the local Gitlance
binary, and register both hooks:

```console
cargo install --path . --locked
task install-githooks
```

The installation is idempotent. If a `commit-msg` or `pre-push` hook already
exists, pre-commit preserves it as a legacy hook and runs it alongside
Gitlance. Running `pre-commit uninstall` restores the previous hooks.

## Commit Requirements

- Use conventional format: `feat:`, `fix:`, `docs:`, etc.
- Include `Signed-off-by: Your Name <email@example.com>`

Example:
```
feat: add custom commit patterns

This allows users to define custom regex patterns.

Signed-off-by: Your Name <email@example.com>
```

## Questions?

Open an issue for discussion.
