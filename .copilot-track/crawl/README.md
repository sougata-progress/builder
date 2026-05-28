# Copilot Crawl – README

This directory (`.copilot-track/crawl/`) stores artefacts produced during the
**AI-assisted crawl-phase** of feature development. It is a lightweight audit
trail that supports code review, traceability, and reproducibility.

---

## What is a "Crawl Phase"?

The crawl phase is the first stage of AI-assisted work on a story or bug:

1. **Explore** – Copilot reads the repository to understand structure and conventions.
2. **Plan** – Copilot produces a structured implementation plan (stored here).
3. **Implement** – Changes are made in a dedicated branch.
4. **Evidence** – Outputs (test results, coverage, lint) are captured and attached to the PR.

Each stage produces an artefact that is committed to this directory before the
PR is opened.

---

## Chain-PRs

Complex features are delivered as a sequence of small, reviewable pull requests
rather than one large PR. Each link in the chain:

- Has a title prefixed with the Jira ID and a sequence indicator, e.g.  
  `BLDR-1234 (1/3): Add DB schema migration`
- Targets the previous branch (not `main`) so diffs stay small.
- References the parent PR in its description.
- Contains its own evidence artefact in this directory.

### Chain-PR naming convention

```
<JIRA-ID>-<nn>   e.g.  BLDR-1234-01, BLDR-1234-02, BLDR-1234-03
```

The final PR in the chain targets `main` and links all preceding PRs.

---

## Evidence in PRs

Every PR description must include an **Evidence** section containing:

| Item | What to include |
|---|---|
| Test results | Paste or link output of `cargo test` / `npm test` |
| Coverage | Report from `cargo tarpaulin` or Karma; must be ≥ 80 % |
| Lint | Output of `run_clippy.sh` and `cargo fmt --check` |
| Crawl plan | Link to the plan file committed in this directory |

Example PR body fragment (HTML format required by repo conventions):

```html
<h2>Evidence</h2>
<ul>
  <li>Tests: all 42 tests pass (<a href=".copilot-track/crawl/BLDR-1234-test-output.txt">log</a>)</li>
  <li>Coverage: 84 % line coverage (<a href=".copilot-track/crawl/BLDR-1234-coverage.html">report</a>)</li>
  <li>Clippy: 0 warnings</li>
  <li>Plan: <a href=".copilot-track/crawl/BLDR-1234-plan.md">BLDR-1234-plan.md</a></li>
</ul>
```

---

## Prompt Usage

Prompts sent to Copilot that produce code or plans **with side-effects** should
be recorded here so that reviewers understand the AI's reasoning.

### What to save

- The exact prompt text (or a concise summary if it is long).
- The Copilot model and date.
- A note on any manual corrections made after the AI output.

### File naming

```
<JIRA-ID>-prompt-<slug>.md
```

Example: `BLDR-1234-prompt-schema-design.md`

### Minimal template

```markdown
# Prompt record – BLDR-1234: Schema design

**Date**: 2026-05-28  
**Model**: Claude Sonnet 4.6 via GitHub Copilot  

## Prompt
<paste prompt here>

## Summary of output
<one paragraph describing what Copilot produced>

## Manual corrections
- <list any hand-edits made after accepting the AI output>
```

---

## Directory Layout

```
.copilot-track/crawl/
├── README.md                    ← this file
├── <JIRA-ID>-plan.md            ← implementation plan per story
├── <JIRA-ID>-prompt-<slug>.md   ← prompt records
├── <JIRA-ID>-test-output.txt    ← captured test output
└── <JIRA-ID>-coverage.html      ← coverage report (generated)
```

Generated files (`.txt`, `.html`) are gitignored by default – commit only the
Markdown artefacts unless a reviewer specifically requests raw output.

---

## Labels

All PRs that contain AI-generated code must carry the `ai-assisted` label.
See the repository's [Copilot instructions](../../.github/copilot-instructions.md)
for the full PR workflow.
