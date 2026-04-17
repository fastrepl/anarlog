# fastrepl.com — Redesign Brief

## Mission
Rebuild `apps/fastrepl/index.html` (and its CSS at `src/input.css`) into a site that feels like a **design and research laboratory**, not a generic SaaS index page. The current site is fine but forgettable — 4 bordered cards in a grid. We want editorial presence, confidence, taste.

## Primary reference
**https://un.ms** — UNMS. Study it. Key qualities to emulate:
- Editorial, monograph-like. Reads more like a lab publication than a portfolio.
- Warm off-white page (`#f2f1ef` is already in our CSS — keep it).
- Tiny typographic meta labels: **"IN PRODUCTION"**, **"Elsewhere–"**, **"Worldwide catalogue"**. These are ornaments, set in muted uppercase or small-caps mono, creating structure without heavy UI.
- Quiet up-right arrow `↗` glyph marks external/outbound links. Consistent everywhere.
- Each project is announced like a published work: name, year, one-sentence description, optional audience tag ("Loved by researchers", "Made for potters", "Early access").
- Mix of serif display + sans body + mono meta — but disciplined, not busy.
- Generous vertical whitespace. Content breathes. No card shadows, no heavy borders, hover is a subtle shift, not a lift.
- Single-column editorial rhythm on mobile, quietly expanding to a wider reading measure on desktop. Not a traditional 3-column grid.

## Other reference sites for inspiration (look these up / draw from memory)
- **rauno.me** — monospace discipline, tight typography, playful microinteractions
- **linear.app/method** — editorial long-form with confident typography
- **rethink.com** — dense catalogue-style index with restraint
- **studio.osmo.supply** — research-house vibe
- **nat.org** — quiet, personal, minimal
- **anthropic.com/claude** — serif-forward editorial calm

Feel free to borrow small ideas from these (not clone them). The destination is **"UNMS-adjacent but clearly fastrepl"**.

## Content to present
Company: **fastrepl** — a small builder collective based in Korea + SF. Open source obsessives.

Tagline options (pick one or write better):
- "An independent software laboratory. We build tools, ship them, and open-source them."
- "Small studio, open source. We make daily software."
- "A software laboratory from Korea + SF. We ship what we'd want to use."

Projects to feature (each: name, year, one-line description, link, optional meta tag):
1. **char** — 2025 — "AI daily notes. Meetings, emails, the lot. Open source." — https://char.com — tag: "8K+ stars on GitHub" or "YC S25"
2. **unsigned** — 2025 — "Free meeting notetaker. Nothing to sign up for." — https://unsigned.char.com — tag: "No account needed"
3. **openbird** — 2025 — (if you can't find a crisp description, write: "An experiment in open communication.") — https://openbird.vercel.app
4. **philo** — 2025 — "A quiet place for ideas." — https://philo.so
5. (optional) Add one "Coming soon" or "In research" item to create the sense of an active lab with work in the oven. Call it **charm** — "A dictation tool for people who talk to their computer." — "In research"

Footer-ish info:
- "Copyright © 2026 fastrepl" or similar
- Links: github.com/fastrepl/char, x.com (if we have one — leave empty if not), a "talk to us" mailto (use `hello@fastrepl.com` as placeholder)
- Maybe a tiny status line: "SEOUL · SF" or "IN PRODUCTION"

## Technical constraints
- **Stack stays the same**: static `index.html` + Tailwind v4 (`@tailwindcss/cli`), built via `npm run build`. No JS framework. No React. No build step beyond Tailwind.
- Preserve `package.json` scripts exactly. `npm run build` must produce `dist/index.html` + `dist/output.css`.
- Preserve Netlify compatibility (`netlify.toml` is there, don't break it).
- Fonts: keep Geist + Geist Mono (already linked). If you want to add a serif, use a high-quality Google font — **Newsreader**, **Instrument Serif**, or **Source Serif 4** are the best candidates for UNMS-adjacent feel. Instrument Serif is my top pick for display headings. Add it via the existing Google Fonts `<link>`.
- Keep/evolve the design tokens already in `src/input.css` (`--color-page`, `--color-fg`, `--color-fg-muted`, `--color-border`, etc.) — add more as needed. Use OKLCH where sensible. Stay within a warm, restrained palette. One accent color is fine if it's used sparingly (a deep ink/navy, a muted rust, a forest green — editorial, not SaaS-blue).
- Fully responsive. Mobile-first. No horizontal scroll.
- Accessibility: real focus states, semantic HTML (`<header>`, `<main>`, `<article>` for projects, `<footer>`), sufficient contrast, `prefers-reduced-motion` respected if you add any transitions.
- Tiny JS is OK only if necessary for a specific effect (e.g., cursor-follow underline) — but prefer CSS.

## What "done" looks like
1. `apps/fastrepl/index.html` is rewritten. Uses semantic elements, reads cleanly.
2. `apps/fastrepl/src/input.css` has evolved tokens and any custom component classes.
3. `npm run build` completes with zero errors from inside `apps/fastrepl/`.
4. Preview screenshot mental check: the page feels quiet, confident, editorial. Someone seeing it thinks "these people have taste."
5. No dead links, no lorem ipsum, no placeholder CSS.
6. The "Projects" section feels like a **curated monograph**, not a dashboard grid.
7. Include at least one piece of typographic delicacy that elevates the page — an oversized em-dash ornament, a tiny running header, a considered colophon at the bottom, numbered entries, something.

## What to AVOID
- Card shadows, gradients, glassmorphism, generic SaaS gradients
- Centered hero with a big "Get Started" button
- Feature grids with icons
- Stock imagery
- Any "Trusted by" logo wall
- Emoji as decoration (very occasionally OK inline in copy if extremely intentional, but default: none)
- Dark mode toggle (not needed for this brief; keep warm light)
- Overuse of `↗` — it's a quiet punctuation mark, not a decoration
- Overwrought animations. Max: subtle opacity fade on hover.

## Process
1. Read `apps/fastrepl/index.html`, `apps/fastrepl/src/input.css`, `apps/fastrepl/package.json`, `apps/fastrepl/netlify.toml` first.
2. Plan the layout in your head (or a short comment in the file header). Then rewrite `index.html` end-to-end — don't patch the existing grid, **throw it out and start from the editorial brief**.
3. Update `src/input.css` tokens/utilities as needed.
4. Run `cd apps/fastrepl && npm install && npm run build` and confirm it succeeds.
5. Open `dist/index.html` mentally (or via `python3 -m http.server`) — verify it looks right.
6. Commit on the current branch `site/fastrepl-redesign` with message: `redesign: editorial fastrepl.com inspired by un.ms`.

## Deliverable
A single commit on `site/fastrepl-redesign`. Tell me when done and list the key design moves you made. Don't ship a PR yet — John reviews first.

Ultrathink on typography and rhythm before you start writing HTML. Taste is the whole point here.
