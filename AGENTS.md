# AGENTS.md — Collaboration rules

Guidance for AI agents and contributors working in this repository. The rules are general and
apply to any task.

## 1. Define the goal first

One-sentence goal plus three non-negotiable constraints (safety, minimality, consistency).
If the goal is unclear, ask before acting.

## 2. Map the whole surface

For any decision or concept, find every place it lives: code, config, scripts, docs, tests,
examples. Use grep to produce the list before editing, then verify each item. Never change only
the spot that was named.

## 3. One standard (SSOT)

A concept has exactly one source of truth. If a second form appears (compatibility layer, alias,
dual layout, dual command), ask whether the two can be unified first. A fallback is allowed only
when unification is impossible, with the reason written down.

## 4. Evidence threshold

"Supported" and "done" must be verified. Unverified work is marked "unverified" or removed.
Verify in increasing cost: compile → smoke test → targeted tests → full run. Skipping levels
requires explicit approval.

## 5. Root cause and siblings

Reproduce before fixing; fix the cause, not the symptom. When touching one site, check the
equivalent sites and fix them in the same pass.

## 6. Impact report

Every delivery ends with three lines: what changed, what may be affected, what was not verified.

## 7. Stop at the boundary

Finish exactly the action that was asked for; do not expand the scope. Long runs, full suites and
release or publish operations only when explicitly requested.

## Project-specific rules

- Declare only verified targets: `windows-amd64`, `linux-amd64`, `linux-arm64`, `android-arm64`.
- Linux is statically linked (musl) and uses Rust's bundled `rust-lld`; no zig.
- One runtime platform layout: `modules` plus `ct.sym` next to the executable (`RAVAC_PLATFORM_DIR` overrides).
- Test data is vendored under `internal/test`; never edit `expected/` baselines by hand.
- Fast verification: `cargo build -p rust-javac` plus a single-file compile smoke test.
  Full and cross builds require an explicit request.
- Before committing, run `git status` and confirm there are no unexpected files; never commit
  `dist/` or `target/`.
