# Anarlog SEO Content Engine

Last updated: 2026-07-07 KST.

This is the operating note for autonomous Anarlog blog work. It complements
`apps/web/content/AGENTS.md`, which remains the source of truth for positioning
and voice.

## Goal

Use Anarlog's existing SEO traction to drive acquisition. Ship useful posts on a
steady cadence, refresh posts that are close to ranking wins, and keep blog media
clean in Supabase.

Anarlog is the SEO-led acquisition product. Char/Charpedia content should remain
founder/product/category narrative unless explicitly requested otherwise.

## Cadence

- Publish or refresh about 6 posts per month.
- Space posts roughly every 4 days.
- Default monthly mix:
  - 3 top-of-funnel posts or refreshes.
  - 2 conversion posts or refreshes.
  - 1 landing-page-support or strategic refresh post.

## Post Types

- Top of funnel: educational/category-intent posts that capture meeting workflow
  searches before readers are vendor-comparing.
- Conversion: competitor alternatives, platform-specific notetaker pages, and
  pages that compare tools while routing readers toward Anarlog's private,
  local-first, bot-free positioning.

## Keyword Workflow

- Use Semrush as the source for keyword and SERP research. Do not fall back to
  Ahrefs.
- Prefer US Semrush data unless a post has an explicit regional target.
- Record the keyword, volume, KD, current Anarlog position when available, and
  target URL in Linear or the PR body.
- Prioritize:
  - Existing Anarlog URLs ranking positions 8-20 for meaningful keywords.
  - Low-KD competitor-alternative terms with clear commercial intent.
  - Platform pages where Anarlog already has a relevant article.
  - Strategic privacy/local/bot-free terms when they strengthen category
    positioning, even if exact volume is small.

## Current First-Cycle Targets

- Top-of-funnel refresh: `meeting-minutes-software`
  - Primary keyword: `meeting minutes software`
  - Semrush signal: volume 1000, KD 39, Anarlog around position 11 before
    refresh.
- Conversion refresh: `otter-ai-alternatives`
  - Primary keyword: `otter ai alternatives`
  - Semrush signal: volume 390, KD 28; variants include
    `otter.ai alternatives` and `alternatives to otter.ai`.
- Conversion refresh: `granola-ai-alternatives`
  - Primary keyword: `granola ai alternatives`
  - Semrush signal: volume 140, KD 13.
- Platform refresh: `best-ai-notetaker-for-microsoft-teams`
  - Primary keyword: `teams ai note taker`
  - Semrush signal: volume 590, KD 43, Anarlog around position 13.
- Platform refresh: `best-ai-notetaker-for-zoom`
  - Primary keyword: `ai notetaker for zoom`
  - Semrush signal: volume 260, KD 35.
- Strategic top-of-funnel:
  `bot-free-ai-meeting-assistants` or a privacy/local variant
  - Primary keywords: `local ai meeting notes`, `private ai notetaker`,
    `bot free ai notetaker`
  - Semrush signal: low/incomplete exact volume, but strong category
    positioning.

## Media Rules

- Cover images should use dynamic OG images. Do not restore stale static
  `cover.png` frontmatter references.
- Body images should live in Supabase Storage bucket `blog`.
- Canonical object path format: `articles/<slug>/<filename>`.
- MDX should reference proxied URLs:
  `/api/assets/blog/articles/<slug>/<filename>`.
- When touching a post, verify body image URLs are not 404/502.
- If existing reachable legacy media is found, copy it into the canonical path
  and update MDX links.

## Napkin Usage

- Use `pnpm --dir apps/web media:napkin` for conceptual diagrams, workflows,
  comparison visuals, and privacy/data-flow figures.
- Napkin URLs expire; always upload generated files to Supabase before using
  them in MDX.
- Do not use Napkin or other generation tools to fake product UI.

## Screenshot Policy

- Product UI screenshots must be real, current screenshots from the app,
  official assets, or user-provided captures.
- If screenshot resources are unavailable, ask for resources or remove the
  screenshot requirement for that section.
- Generated images are acceptable only for conceptual/non-UI visuals.

## Validation Before PR

For blog/content-only changes:

- `dprint check --config dprint.json <changed files>`
- `pnpm --dir apps/web typecheck`
- Verify referenced body images return `200` through the production asset proxy
  when they are already deployed assets.

For changes that affect scripts/routes/build behavior:

- Run the focused script test or `node --check` where applicable.
- Run `infisical run --projectId 87dad7b5-72a6-4791-9228-b3b86b169db1 --path /anarlog/web --env prod -- pnpm --dir apps/web build` when a full web build is needed.

## Tracking

Use the Linear project `Anarlog SEO & Content Engine` for task state.

Known current blockers:

- Semrush connector can report keyword/project data but cannot create or
  configure a Semrush project for `anarlog.so`.
- Some older posts have missing body screenshots. Do not fabricate product UI;
  ask for screenshots or remove/replace the image with a legitimate conceptual
  visual.
