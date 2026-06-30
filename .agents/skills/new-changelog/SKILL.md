---
name: new-changelog
description: Create the next desktop changelog entry when asked to add a changelog file or prepare the next release note under `packages/changelog/content`. Use this when the task is specifically about determining the next version and creating the markdown entry.
---

Determine the next desktop version by inspecting `.github/workflows/desktop_cd.yaml` and running:

```bash
doxxer --config doxxer.desktop.toml current
doxxer --config doxxer.desktop.toml next patch
```

Create the new markdown file in `packages/changelog/content` for that version.

Each changelog file must start with frontmatter that includes both `date` and
`summary`:

```md
---
date: "YYYY-MM-DD"
summary: "One concise, user-facing sentence for the changelog index preview."
---
```

Keep `summary` plain text. Do not use markdown or custom tags in it. The web
changelog index renders this field directly, so it should describe the release
at a glance without leaking implementation details.
