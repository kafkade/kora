---
name: pr-ready
description: >
  Prepare a pull request: generate a PR description using the repo's template,
  update the Unreleased section of CHANGELOG.md with user-facing changes, and
  copy the PR description to the clipboard. Invoke when the user asks to
  "generate a PR description", "describe this PR", "write PR notes",
  "prepare a PR", "pr ready", or "prep this PR".
---

# PR Ready — Description + Changelog

Prepare a branch for pull request: generate a PR description from the diff AND update the changelog, then copy the PR description to the clipboard.

## Steps

### Phase 1: Gather context

1. **Detect the base branch**
   Run `git remote show origin` or check for `main` / `master` to determine the default branch.

2. **Identify the current branch**
   Run `git branch --show-current` to get the feature branch name.

3. **Gather the diff context**
   - Run `git log <base>..<current> --oneline` to get the commit list.
   - Run `git diff <base>..<current> --stat` to get the file change summary.
   - For each changed source file, read the diff to understand what changed.
   - Skip binary files, lock files, and generated files.
   - Note which modules are affected: `core/`, `playback/`, `backend/`, `providers/`, `tui/`, `ipc/`, `docs/`.

4. **Read the PR template**
   - Look for `.github/pull_request_template.md` in the repository.
   - Use its exact structure (sections, checkboxes) as the PR description format.
   - If no template exists, use a sensible default (Description, Changes, Checklist).

5. **Read the current changelog**
   - Read `CHANGELOG.md` and note the existing `## [Unreleased]` section contents.
   - Understand the Keep a Changelog format used (Added, Changed, Deprecated, Removed, Fixed, Security).

### Phase 2: Run quality checks

1. **Verify the branch passes checks** (for the PR checklist)
   Run the standard Rust checks:
   - `cargo fmt -- --check`
   - `cargo clippy -- -D warnings`
   - `cargo test`
   - Note which checks pass/fail to fill in the checklist accurately.

### Phase 3: Generate the PR description

1. **Write the PR description** using the template structure:
   - **Description section**: Write a clear summary of what the PR does. Include:
     - A one-line summary of the purpose
     - A "What's included" subsection listing key changes with brief explanations
     - Reference specific files/modules only when it adds clarity
   - **Related Issues**: Check commit messages for issue references (#123). If none, leave placeholder.
   - **Type of Change**: Check the appropriate box(es) based on diff content.
   - **Module**: Note which modules were touched (`playback/`, `tui/`, `providers/`, etc.)
   - **Checklist**: Mark items based on the results from the quality checks step.

2. **Quality guidelines for the PR description**
   - Do NOT reference internal planning documents (roadmap phases, ADR numbers) — describe actual changes
   - Write from the user/contributor perspective
   - Be specific about what was added/changed
   - Keep the description concise but complete — aim for 15-30 lines

### Phase 4: Update the changelog

1. **Identify user-facing changes** from the diff. A change is user-facing if it:
   - Adds a feature the user can see or interact with
   - Fixes a bug the user could encounter
   - Changes behavior the user would notice (playback, UI, CLI commands, keybindings)
   - Adds or changes configuration options
   - **Is NOT user-facing**: refactoring, CI changes, test additions, internal restructuring, dependency updates, code style fixes, documentation-only changes

2. **Categorize each change** using Keep a Changelog categories:
   - **Added** — new features or capabilities
   - **Changed** — changes to existing functionality
   - **Deprecated** — features that will be removed
   - **Removed** — features that were removed
   - **Fixed** — bug fixes
   - **Security** — vulnerability fixes

3. **Update the `## [Unreleased]` section** of `CHANGELOG.md`:
   - **Append** new entries to the existing Unreleased section — do NOT delete what's already there
   - If a category header (e.g., `### Added`) already exists with entries, add new entries below the existing ones
   - If a category header doesn't exist yet, add it
   - Write entries as concise, user-facing descriptions — no implementation details
   - Each entry starts with a `-` list marker
   - Do NOT include entries for: CI changes, refactoring, dependency bumps, test-only changes, documentation-only changes

4. **Changelog entry style guide**
   - ✅ Good: `- Internet radio playback with Radio Browser station directory`
   - ✅ Good: `- Fixed audio dropout when seeking near end of track`
   - ✅ Good: `- 10-band graphic EQ with Rock, Jazz, and Classical presets`
   - ❌ Bad: `- Refactored decoder module` (not user-facing)
   - ❌ Bad: `- Added unit tests for ring buffer` (not user-facing)
   - ❌ Bad: `- Implements Phase 3 from roadmap` (references internals)

### Phase 5: Output

1. **Copy the PR description to the clipboard**
   - Use PowerShell `Set-Clipboard` (Windows), `pbcopy` (macOS), or `xclip` (Linux)
   - Confirm to the user that the description has been copied

2. **Suggest a PR title**
   - Based on the changes, suggest a conventional-commit-style PR title
   - Format: `feat: add internet radio playback` or `fix: correct gapless transition gap`
   - For module-scoped changes: `feat(playback): add 10-band graphic equalizer`

3. **Show a summary** to the user:
   - The suggested PR title
   - Confirmation that the PR description is on the clipboard
   - A summary of what was added to CHANGELOG.md (list the new entries)
   - Note any changelog entries that were already present and preserved
