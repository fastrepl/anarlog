# Napkin Figures for Blog Posts

Use Napkin for blog conceptual figures: workflows, privacy/data-flow diagrams,
decision trees, comparison visuals, and abstract explainers. Blog diagrams
should come from Napkin rather than hand-rolled SVG/PNG assets unless the user
explicitly asks for a non-Napkin figure. Do not use it to fake product UI. If a
post needs product screenshots and official/current screenshots are not
available, ask for real screenshots or app access.

Generated Napkin file URLs expire after 30 minutes, so every accepted figure
must be downloaded immediately and rehosted in the Supabase `blog` bucket under
`articles/<slug>/...`.

## Batch usage (preferred)

Figures are declared per article in `content/articles/figures.json`, keyed by
slug. `napkin-batch.mjs` reads that manifest, skips figures already present in
the bucket, and generates only what is missing.

```bash
infisical run --silent \
  --env=prod \
  --projectId=87dad7b5-72a6-4791-9228-b3b86b169db1 \
  --path=/anarlog/web \
  -- pnpm -F @anlg/web media:figures
```

Add `--slug <slug>` to limit it to one article, `--upsert` to regenerate
existing figures, or `--dry-run` to print the requests without calling Napkin.

Declare a figure like this:

```json
{
  "my-post-slug": [
    {
      "filename": "capture-flow.png",
      "content": "Meeting audio\nLocal transcription\nMarkdown note",
      "context": "Horizontal flow diagram for an Anarlog blog post.",
      "visualQuery": "flowchart",
      "orientation": "horizontal",
      "width": 1200
    }
  ]
}
```

An empty array marks a post as deliberately figure-less.

### The check

```bash
pnpm -F @anlg/web media:figures:check
```

Needs no credentials — it only makes public HEAD requests — so `web_ci` runs it
on every PR touching `apps/web/`. It **fails** when an article has no manifest
entry, or when the manifest names a slug that no longer exists. A declared but
not-yet-generated figure only warns, so a generation backlog does not turn CI
red; pass `--strict` to gate on that too.

The effect is that adding a new post forces a decision about its figure, without
requiring Napkin credentials in CI.

## Single-figure usage

Run through Infisical so the script can read `NAPKIN_API_TOKEN`,
`SUPABASE_URL`, and `SUPABASE_SERVICE_ROLE_KEY`:

```bash
infisical run --silent \
  --env=prod \
  --projectId=87dad7b5-72a6-4791-9228-b3b86b169db1 \
  --path=/anarlog/web \
  -- pnpm --dir apps/web exec node scripts/napkin-to-supabase.mjs \
    --slug meeting-minutes-software \
    --filename meeting-minutes-workflow.png \
    --content-file /tmp/meeting-minutes-workflow.txt \
    --context "Anarlog blog figure for private, bot-free meeting notes" \
    --visual-query flowchart \
    --orientation horizontal \
    --width 1200
```

The script prints:

- the Napkin request ID
- the selected generated file metadata
- the Supabase storage path
- the public Supabase URL
- the `/api/assets/blog/...` media URL to use in MDX

## Defaults

- Format defaults from the filename extension.
- Style defaults to Anarlog's Napkin brand ID:
  `CDQPRVVJCSTPRBBCD5Q6AWSDE8S0`.
- Language defaults to `en-US`.
- Upload path is always `articles/<slug>/<filename>`.
- Uploads do not overwrite existing objects unless `--upsert` is passed.

## Prompt Shape

Keep `--content` or `--content-file` limited to the text that should appear in
the visual. Put editorial instructions, audience, and brand context in
`--context`.

Good content example:

```text
Meeting recording
Local transcription
AI summary
Human review
Published meeting minutes
```

Good context example:

```text
Create a clean horizontal workflow diagram for an Anarlog blog post. The figure
should explain a private, bot-free meeting minutes workflow for teams evaluating
meeting minutes software.
```
