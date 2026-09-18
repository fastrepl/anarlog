# Fork overlay

This fork of `fastrepl/anarlog` renames the product and drops two apps. Both are
mechanical, so neither is stored as hand-edited divergence: they are re-derived
after every upstream merge.

## Why

Measured against `upstream/main`, the fork changed 1350 files and upstream had
touched 1330 of them in the preceding six months — a 98.5% overlap. Left as
hand edits, that is a conflict on almost every merge. Two classes dominate:

| Source | Files | Why git cannot help |
| --- | --- | --- |
| `apps/web` + `apps/mobile` deleted | 870 | `modify/delete` has no automatic resolution |
| Files differing only by the product name | 134 | conflicts whenever upstream edits the same copy |

Both are now derived, so a merge only stops on real behavioural divergence.

## Pieces

- **`brand.json`** — the only place the name lives. `protected` lists substrings
  that must survive verbatim: URL schemes, bundle ids and package names are
  identity, not copy, and renaming them would orphan existing installs.
- **`apply.mjs`** — rewrites the tracked tree from `brand.json`. Idempotent, so
  running it on an already-branded tree is a no-op. `--check` fails instead of
  writing, for CI.
- **`removed-paths.txt`** — path prefixes this fork deleted on purpose.

Licences, notices and `packages/changelog/content` are never rewritten: a licence
names the work it was granted for, and the changelog records what upstream
actually shipped under its own name.

## Use

```bash
scripts/merge-upstream.sh          # merge upstream/main, resolve both classes
node branding/apply.mjs            # re-derive the brand by hand
node branding/apply.mjs --check    # fail if the tree has drifted
```

To change the name, edit `brand.json` and run `apply.mjs` — nothing else.
