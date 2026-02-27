# Playbook: Coding Agent Workflow

Development workflow for coding agents (Claude Code, Cursor, etc.) working on the AI Proxy Gateway.

## Before Starting

1. **Read `CLAUDE.md`** -- Project context, commands, rules, and code style conventions.
2. **Read `AGENTS.md`** -- Architecture overview, crate responsibilities, API endpoints, and provider matrix.
3. **Check `docs/specs/_index.md`** -- Review existing specs to understand what has been built and what is in progress.
4. **Check `docs/playbooks/`** -- Relevant playbooks for common tasks (adding providers, translators, etc.).

## Workflow by Task Type

### New Features

1. **Create a spec** following the [create-new-spec.md](create-new-spec.md) playbook.
2. Fill in the PRD (problem, goals, non-goals, user stories).
3. Fill in the Technical Design (implementation details, API design, task breakdown).
4. Get the spec reviewed/approved (move status from Draft to Active).
5. Implement following the technical design.
6. Run quality checks:
   ```sh
   make lint   # cargo fmt --check + cargo clippy -- -D warnings
   make test   # cargo test --workspace
   ```
7. Move the spec from `active/` to `completed/` and update `_index.md`.
8. Submit using `/ship`:
   ```
   /ship "feat: add support for new-provider streaming"
   ```

### Bug Fixes

1. Identify related spec(s) in `docs/specs/`.
2. Reproduce the issue and understand the root cause.
3. Implement the fix.
4. Add a test that covers the bug scenario.
5. Run quality checks:
   ```sh
   make lint
   make test
   ```
6. Update documentation if the fix changes observable behavior.
7. Submit using `/ship`:
   ```
   /ship "fix: correct SSE parsing for multi-line data fields"
   ```

### Refactoring

1. Check which specs are affected by the refactor.
2. Make changes incrementally, verifying tests pass at each step.
3. Run quality checks:
   ```sh
   make lint
   make test
   ```
4. Update affected reference docs in `docs/reference/`.
5. Submit using `/ship`:
   ```
   /ship "refactor: extract credential routing into CredentialRouter"
   ```

### Documentation Changes

1. Update the relevant files in `docs/`.
2. Verify links and references are correct.
3. Submit using `/ship`:
   ```
   /ship "docs: update API endpoint reference"
   ```

### Adding Tests

1. Identify gaps in test coverage.
2. Add tests in the appropriate crate's test module.
3. Run `make test` to verify.
4. Submit using `/ship`:
   ```
   /ship "test: add integration tests for translator registry"
   ```

### Handling Dependabot PRs

Use the `/deps` command to manage Dependabot pull requests:

1. **Check status**: `/deps` -- Lists all open Dependabot PRs grouped by CI status.
2. **Merge passing PRs**: `/deps merge` -- Squash-merges all PRs with green CI.
3. **Fix failing PRs**: `/deps fix` -- Checks out failing PRs, fixes build issues, pushes, and merges on CI pass.
4. **Manual update**: `/deps update` -- Runs `cargo update`, checks lint+test, commits if passing.

## Commit & Push

Use `/ship` for all commit and push operations. It handles:
- Formatting and linting (`make fmt` + `make lint`)
- Testing (`make test`)
- Documentation sync checks
- Spec association checks
- Commit message generation (conventional commits)
- Push and PR creation

Use `/ship --no-pr` when you only need to commit and push without creating a PR.

## Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):

| Prefix     | Usage                                          |
|------------|------------------------------------------------|
| `feat:`    | New feature or capability                      |
| `fix:`     | Bug fix                                        |
| `docs:`    | Documentation only changes                     |
| `refactor:`| Code change that neither fixes a bug nor adds a feature |
| `test:`    | Adding or correcting tests                     |
| `chore:`   | Build process, CI, or auxiliary tool changes    |

Examples:
```
feat: add Gemini provider streaming support
fix: correct SSE parsing for multi-line data fields
docs: update API endpoint reference
refactor: extract credential routing into CredentialRouter
test: add integration tests for translator registry
chore: update dependencies to latest versions
```

## Quality Gates

Before every commit, ensure:

1. **`make lint` passes** -- Runs `cargo fmt --check` and `cargo clippy --workspace -- -D warnings`.
2. **`make test` passes** -- Runs `cargo test --workspace`.
3. **No secrets committed** -- Never commit `config.yaml`, `.env`, API keys, or credentials. Use `config.example.yaml` as a template.
4. **`Cargo.lock` is committed** -- This is a binary project; the lock file must be tracked.

Note: The pre-commit hook in `.claude/settings.json` enforces `make lint && make test` automatically on `git commit`.

## Key Paths

| Path                     | Purpose                                        |
|--------------------------|------------------------------------------------|
| `crates/core/`           | Foundation types, config, errors, auth, metrics |
| `crates/provider/`       | Provider executors, credential routing, SSE     |
| `crates/translator/`     | Format translation between provider APIs        |
| `crates/server/`         | Axum router, handlers, middleware, dispatch     |
| `src/`                   | Binary entry point                              |
| `docs/specs/`            | SDD spec registry                               |
| `docs/reference/`        | API and architecture reference docs             |
| `docs/playbooks/`        | How-to guides (this directory)                  |

## Common Tasks Quick Reference

| Task                  | Playbook                                        |
|-----------------------|-------------------------------------------------|
| Add a new provider    | [add-provider.md](add-provider.md)              |
| Add a translator      | [add-translator.md](add-translator.md)          |
| Create a new spec     | [create-new-spec.md](create-new-spec.md)        |

## Tips for Coding Agents

- Read the source code before making changes. Understand existing patterns.
- Follow the existing code style (Rust Edition 2024, `snake_case` functions, `PascalCase` types).
- Use `thiserror` for library error types, `anyhow` for application-level errors.
- Use `async-trait` for async trait methods.
- Use `serde` with `serde_json` and `serde_yml` for serialization.
- Configuration uses `arc-swap` for hot-reload. Access config via `ArcSwap<Config>`.
- When in doubt, look at how existing providers and translators are implemented and follow the same pattern.
