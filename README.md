<!--
SPDX-FileCopyrightText: Canonical Ltd.

SPDX-License-Identifier: Apache-2.0
-->

# gitlance

[![Validate](https://github.com/agherzan/gitlance/actions/workflows/validate.yml/badge.svg)](https://github.com/agherzan/gitlance/actions/workflows/validate.yml)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![REUSE](https://api.reuse.software/badge/github.com/agherzan/gitlance)](https://api.reuse.software/info/github.com/agherzan/gitlance)

A GitHub Action for validating Git commit messages and history. Checks for WIP/fixup commits, Signed-off-by trailers, and conventional commit format.

## Features

- **Fast execution** - Written in Rust for minimal overhead
- **Extensible** - Easy to add new checks in the future
- **Clear feedback** - GitHub Actions annotations for failed checks
- **Automatic failure** - Fails the workflow step when checks don't pass
- **Default all-checks mode** - Runs all validations in a single invocation
- **Auto-detection** - Automatically detects base/head SHAs from PR context

## Usage

### Basic Usage

Add to your workflow:

```yaml
steps:
  - uses: actions/checkout@v4
    with:
      fetch-depth: 0  # Required to access commit history
  - uses: agherzan/gitlance@v1
```

This will run all checks by default in your PR context.

### Specific Check

Run only a single check:

```yaml
steps:
  - uses: actions/checkout@v4
    with:
      fetch-depth: 0
  - uses: agherzan/gitlance@v1
    with:
      check: wip-fixup
```

### Manual SHA Specification

For non-PR workflows (e.g., push events), provide SHAs explicitly:

```yaml
steps:
  - uses: actions/checkout@v4
    with:
      fetch-depth: 0
  - uses: agherzan/gitlance@v1
    with:
      base-sha: ${{ github.event.before }}
      head-sha: ${{ github.sha }}
```

Note: In PR context, SHAs are auto-detected and this is not needed.

### Using the Output

The action automatically fails the workflow when checks fail. Use the `passed` output for conditional logic:

```yaml
steps:
  - uses: actions/checkout@v4
    with:
      fetch-depth: 0
  - id: checks
    uses: agherzan/gitlance@v1
    continue-on-error: true  # Don't fail workflow immediately

  - name: Post comment on validation failure
    if: steps.checks.outputs.passed == 'false'
    uses: actions/github-script@v7
    with:
      script: |
        await github.rest.issues.createComment({  // Wait for comment to post before failing
          issue_number: context.issue.number,
          owner: context.repo.owner,
          repo: context.repo.repo,
          body: '❌ Commit validation failed. Please review the checks.'
        })

  - name: Fail workflow if checks failed
    if: steps.checks.outputs.passed == 'false'
    run: exit 1
```

## Available Checks

### wip-fixup
Detects Work-in-Progress, fixup, squash, and amend commits. Fails if any commit message starts with:
- `fixup!`
- `squash!`
- `amend!`
- `WIP` or `wip` (followed by space or colon)

### signed-off-by
Validates that all commits have a valid `Signed-off-by` trailer in the format:
```
Signed-off-by: Name <email@domain>
```

### conventional-commits
Enforces conventional commit format for all commits. Valid types:
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation
- `style` - Code style
- `refactor` - Refactoring
- `perf` - Performance
- `test` - Tests
- `build` - Build system
- `ci` - CI/CD
- `chore` - Chores
- `revert` - Revert

Format: `type(scope)?: description`

Breaking changes are indicated with `!` before the colon:
- `feat!: breaking change in API`
- `fix(auth)!: change authentication flow`

Examples:
- `feat: add new feature`
- `fix(api): resolve authentication bug`
- `docs: update README`
- `feat(cli): add verbose output flag`

### all
Runs all available checks. This is the default if no check is specified.

## Example Workflows

### Check all commits on PR

```yaml
name: Commit Checks

on:
  pull_request:
    branches: [ main ]

jobs:
  checks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: agherzan/gitlance@v1
```

### Check with strict conventional commits

```yaml
steps:
  - uses: actions/checkout@v4
    with:
      fetch-depth: 0
  - uses: agherzan/gitlance@v1
    with:
      check: conventional-commits
```

### Multiple checks with different steps

Run all checks even if some fail, then fail the workflow if any failed:

```yaml
steps:
  - uses: actions/checkout@v4
    with:
      fetch-depth: 0

  - uses: agherzan/gitlance@v1
    id: wip-check
    continue-on-error: true
    with:
      check: wip-fixup

  - uses: agherzan/gitlance@v1
    id: signoff-check
    continue-on-error: true
    with:
      check: signed-off-by

  - name: Fail if any check failed
    if: steps.wip-check.outputs.passed == 'false' || steps.signoff-check.outputs.passed == 'false'
    run: exit 1
```

## Installation

The action automatically downloads the pre-built binary for the specified version. No additional setup is required.

**Note:** Prebuilt binaries are only available for immutable release tags (e.g., `@v1.2.3`). Major tags like `@v1` will build from source on each run.

## Building Locally

### Using Task

```bash
# Run all validation checks
task validate

# Build release binary
task build-release

# Run tests
task test

# Show all available tasks
task help
```

### Using Cargo Directly

```bash
# Build the project
cargo build --release

# Run tests
cargo test --verbose

# Run specific check on your repo
./target/release/gitlance wip-fixup \
  --base-sha <base_sha> \
  --head-sha <head_sha> \
  --repo .
```

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
