# Walkthrough accessibility pass — 2026-05-31

Acting on a 6-journey "average user" evaluation of `docs/walkthroughs/`. Logical
flow scored well (~4/5); accessibility lagged (~3.2/5), held back by missing
*connective tissue* rather than broken journeys.

## Findings (consensus across 6 independent evaluators)

1. Undefined jargon on first use — 6/6 (namespace, adapter, manifest, the
   use/activate/materialize trio). Named the biggest blocker in 5/6 verdicts.
2. No failure/recovery paths — 5/6 (happy-path only; no undo).
3. Cold-start prereqs / assumes a prior journey — 4/6 (updating-a-profile,
   install-a-skill fail at command one for a standalone reader).
4. No collection-level index / reading order.
5. Smaller: updating-a-profile shows no command output; snapshot Step 5 self-
   contradicts; snapshot calls shipped `aenv diff` a "Phase 5" feature; some
   docs lack an explicit "you're done" close.
6. README.md:10 still says "Latest release is v0.3.0" (shipped v0.3.1).

## Plan

- [x] Author `docs/walkthroughs/README.md` — index (recommended reading order)
      + canonical **Glossary** (terms verified against `aenv --help`).
- [x] Bump `README.md` version string v0.3.0 → v0.3.1.
- [x] Per journey (6): insert a short **Concepts** box (links glossary) + an
      **If something goes wrong** recovery section (commands verified to exist),
      and apply that doc's specific fixes (prereq fallback, contradictions,
      "done" close, output samples).
- [x] Verify: intra-doc links/anchors resolve; recovery commands match real CLI
      help; consistency of inserted blocks across all six.

## Results

- New `docs/walkthroughs/README.md` index + glossary; all 6 journeys link it.
- All 6 journeys now carry a Concepts box and a recovery section (verified:
  6/6 each). All relative links resolve; `#glossary` and README
  `#global-namespaces` anchors confirmed present.
- Specific fixes landed: snapshot Step 5 self-contradiction rewritten; snapshot
  "(Phase 5)" tag dropped (`aenv diff` is shipped); install-a-skill prereq now
  has an inline `aenv create` fallback; cold-start docs link setup-first.
- No shell command or expected-output block was altered (drift guard held):
  the only removed lines were narrative prose.
- Deferred: adding captured command output to `updating-a-profile` (would mean
  editing existing output blocks; skipped to avoid drift, not fabricated).

## Recovery commands (verified to exist via `aenv <cmd> --help`)

- `aenv deactivate` — reverse activate, restore backups, delete `.aenv-state/`,
  leaves the `.aenv` pin.
- `aenv unpin` — remove the `.aenv` pin (runs deactivate first if active).
- `aenv restore` — copy the latest `.aenv-state/backup/<ts>/` back when
  deactivate didn't run cleanly.
- `aenv skill remove <name> --ns <ns>` (+ `aenv cache prune` for imported).
- `aenv fork` — detach a file/project from namespace management.
