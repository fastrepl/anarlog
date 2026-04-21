---
version: alpha
name: Char
description: >-
  A carefully crafted notebook that lives in the digital space.
  AI daily notes that remember and act — Char records meetings without
  bots, pulls action items from emails, and hands the rest off to agents.
  The brand reads as calm: warm neutrals, breathing room, mono display
  type, and notebook-style line work.
colors:
  # Grey scale foundation — authoritative values are oklch in styles.css.
  # Hex here are sRGB approximations for tooling.
  grey-900: "#3d3830"
  grey-700: "#726c64"
  grey-500: "#c1bcb6"
  grey-300: "#ebe9e4"
  grey-100: "#f3efe9"

  # Semantic palette
  page: "#f2f1ef"
  surface: "#ffffff"
  surface-subtle: "{colors.grey-100}"

  fg: "{colors.grey-900}"
  fg-muted: "#57534e"
  fg-subtle: "{colors.border}"

  border: "{colors.grey-500}"
  border-subtle: "{colors.grey-300}"
  border-bright: "#968775"
  border-active: "{colors.grey-300}"

  brand-dark: "#57534e"
  brand-yellow: "#f4e6a5"

  # CTA gradient anchors (Tailwind stone scale)
  stone-500: "#78716c"
  stone-600: "#57534e"

typography:
  h1:
    fontFamily: Geist Mono
    fontSize: 3.2rem
    fontWeight: 500
    lineHeight: 1.3
    letterSpacing: -0.02em
  h2:
    fontFamily: Geist Mono
    fontSize: 2.2rem
    fontWeight: 500
    lineHeight: 1.3
    letterSpacing: -0.02em
  h3:
    fontFamily: Geist
    fontSize: 1.5rem
    fontWeight: 600
    lineHeight: 1.3
  h4:
    fontFamily: Geist
    fontSize: 1.25rem
    fontWeight: 600
    lineHeight: 1.3
  body-lg:
    fontFamily: Geist
    fontSize: 1.3rem
    lineHeight: 1.5
  body-md:
    fontFamily: Geist
    fontSize: 1rem
    lineHeight: 1.6
  body-sm:
    fontFamily: Geist
    fontSize: 0.875rem
    lineHeight: 1.5
  label-caps:
    fontFamily: Geist Mono
    fontSize: 0.75rem
    fontWeight: 600
    letterSpacing: 0.05em
  button:
    fontFamily: Geist Mono
    fontSize: 0.875rem
    fontWeight: 500
  editorial:
    fontFamily: Fraunces
    fontSize: 1.25rem
    lineHeight: 1.5
  editorial-italic:
    fontFamily: Instrument Serif
    fontSize: 1.25rem
    fontWeight: 400
  mono-code:
    fontFamily: Geist Mono
    fontSize: 0.875rem

rounded:
  xs: 4px
  sm: 6px
  md: 8px
  lg: 12px
  xl: 16px
  full: 9999px

spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 32px
  "2xl": 48px
  "3xl": 64px

components:
  button-primary:
    backgroundColor: "{colors.stone-600}"
    textColor: "#ffffff"
    typography: "{typography.button}"
    rounded: "{rounded.full}"
    padding: 16px
    height: 32px
  button-primary-hover:
    backgroundColor: "{colors.stone-500}"
  button-primary-active:
    backgroundColor: "{colors.stone-600}"

  button-ghost:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg-muted}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.md}"
    padding: 16px
    height: 36px
  button-ghost-hover:
    backgroundColor: "{colors.grey-100}"

  nav-link:
    textColor: "{colors.fg-muted}"
    typography: "{typography.body-sm}"
  nav-link-hover:
    textColor: "{colors.fg}"

  section-label:
    textColor: "{colors.fg-subtle}"
    typography: "{typography.label-caps}"

  card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.md}"
    padding: 16px
  card-feature:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.md}"
    padding: 32px

  hero-container:
    rounded: "{rounded.lg}"
    height: 88vh
---

## Overview

Char is a digital notebook — a quiet space for thinking, recording, and handing work off to AI. The marketing site should feel like **a carefully crafted notebook lives in the digital space**: horizontal lines and grids suggest paper, handwritten accents cut in from elsewhere, structured interfaces rest beside warm typography.

Char values slow exploration and breathing room. Layouts carry a lot of air, colors recede, motion is restrained. The hero line — **"AI daily notes that remember and act"** — drives the whole brand posture: Char is the note-taker, not the performer. It records meetings without bots, pulls action items from your emails, and hands the rest off to AI agents like Claude or Cursor.

The homepage currently walks four promises: *Record your day → Summarise meetings → Execute tasks → Private by design.* Everything else in the system supports that narrative.

## Colors

The palette is warm neutral — an oklch-derived grey scale with off-white and stone tones. The one moment of warmth is `brand-yellow`, reserved for the hero and footer wash. CTAs use a stone gradient (`stone-600 → stone-500`). Everything else recedes.

### Grey scale

The grey scale is authored in oklch for perceptual uniformity. The hex values in the front matter are sRGB approximations — for precise rendering, reference the oklch tokens in `src/styles.css`.

| Token | oklch | Role |
|---|---|---|
| `grey-900` | `oklch(0.3 0.0197 81.53)` | Primary text |
| `grey-700` | `oklch(0.4922 0.0127 67.79)` | Secondary text (alt) |
| `grey-500` | `oklch(0.7782 0.0018 67.8)` | Default border |
| `grey-300` | `oklch(0.9213 0.0027 106.45)` | Hairline border |
| `grey-100` | `oklch(0.9558 0.0045 78.3)` | Muted surface |

### Usage rules

- **Never introduce a color outside the palette** without updating the token set first.
- `brand-dark` drives checked states and emphasis. CTAs always use the stone gradient — never a custom-token gradient.
- `brand-yellow` is the warm wash at the top of marketing pages and the footer. **Not for buttons or text.**
- Tailwind's `neutral-*` and `stone-*` scales appear in components as fallback; the semantic tokens above take precedence for brand-facing UI.
- `fg-subtle` deliberately aliases `border` so placeholder text and borders share the same value.

## Typography

Six typefaces are loaded. Three carry distinct roles; the rest are special-purpose.

| Face | Font | Role |
|---|---|---|
| Mono | Geist Mono | Display headings (h1, h2), button labels, code |
| Sans | Geist | Body copy, UI labels, navigation, subheadings |
| Serif | Fraunces | Wordmark weight, editorial pull-quotes |
| Serif (italic) | Instrument Serif | Italic editorial accents |
| Display | Redaction | Decorative / redacted text effects |
| Sans (alt) | SF Pro | System-matching UI |

### Hierarchy principles

- **h1 and h2 are monospace by default** — Geist Mono at weight 500, `tracking-tight` (−0.02em), line-height 1.3. Serif is used selectively for editorial or brand moments, never for all headings.
- **Pair weight contrasts**: heavy display (`semibold`/`bold`) with light body (`normal`). Never two heavy weights adjacently.
- **Letter spacing**: tight on large display type, wide (`tracking-wider`) on all-caps labels and category tags.
- **Minimum readable size**: 14px for body; 12px only for all-caps labels or metadata.

### Base rules (from `styles.css`)

- `html, body` → `font-sans`
- `h1, h2` → `font-mono`, weight 500, tracking −0.02em, line-height 1.3
- `h3–h6, p, span, li` → `font-sans`
- `p` → 1.3rem / 1.5 line-height
- `button, [role="button"]` → `font-mono`

## Layout

The marketing site uses a 3-column layout on large screens: left sidebar, center content, right panel.

### Breakpoints

| Name | Value | Purpose |
|---|---|---|
| `md` (Tailwind) | 768px | Tablet header bar |
| `xl` (Tailwind) | 1280px | Desktop sidebar appears |
| `laptop` | 1152px | General responsive breakpoint |
| `wide` | 1400px | Wide sidebar/panel sizing |

### Dimensions

| Purpose | Value |
|---|---|
| Outer max-width | 1800px (3-column wrapper) |
| Content max-width | `max-w-6xl` (footer, header) |
| Mobile top bar height | 56px |
| Scroll margin (anchors) | 69px |
| Section vertical padding | 48px mobile / 64px desktop |
| Card padding — compact | 16px |
| Card padding — feature | 32px |
| Hero min-height | 88vh |
| Hero mock height | 90% of notebook panel |

### Responsive tiers

| Range | Layout |
|---|---|
| `< 768px` | Fixed top bar + hamburger dropdown, single column |
| `768 – 1280px` | Fixed horizontal header bar, single column |
| `1280px +` | Sticky left sidebar + content + sticky right panel |

### Background patterns

Decorative backgrounds used on sections and cards. All use 24px spacing (23px gap + 1px line).

| Class | Pattern |
|---|---|
| `.bg-lined-notebook` | Subtle horizontal lines |
| `.bg-lined-notebook-dark` | Default horizontal lines |
| `.bg-lined-notebook-bright` | Accent horizontal lines (used behind the hero mock) |
| `.bg-dotted`, `.bg-dotted-dark` | Dot grid |
| `.bg-grid`, `.bg-grid-dark` | Full grid |

### Brand yellow wash

Marketing pages (homepage, product pages) carry a warm yellow wash at the top, rendered as two overlapping layers:

1. A CSS gradient from `brand-yellow` to transparent, covering `h-[180vh]`.
2. A repeating noise texture at 30% opacity, masked to fade out downward. Noise is generated from `src/lib/brand-noise.ts`.

Resource pages (docs, blog, changelog) skip this wash. The footer mirrors the effect in reverse: transparent to `brand-yellow` with the same noise.

## Elevation & Depth

Char avoids visual weight. Shadows are sparing and always subtle.

| Token | Value | Use |
|---|---|---|
| `shadow-ring` | `0 0 0 1px {colors.border}` | Default card/panel outline |
| `shadow-ring-left` | `-1px 0 0 1px {colors.border}` | Left-edge only outline |
| `shadow-md` | Tailwind default | CTA buttons at rest |
| `shadow-lg` | Tailwind default | CTA hover, dropdown menus |
| `shadow-xl` | Tailwind default | Hero mock, elevated cards |

**Prefer `shadow-ring` over CSS `border`** when an element already has box-shadow — avoids double-border stacking.

No drop-shadows on text. No inset shadows. Depth comes from borders and stacking, not from glow.

## Shapes

| Token | Use |
|---|---|
| `rounded.xs` (4px) | Tight UI elements — badges, dropdown panel corners |
| `rounded.sm` (6px) | Keyboard key pills, small chips |
| `rounded.md` (8px) | Cards, inputs, dropdown rows |
| `rounded.lg` (12px) | Modals, large cards, hero containers |
| `rounded.xl` (16px) | Hero mock window, prominent panels |
| `rounded.full` | Pill buttons, avatars, tags |

### Bracket motif

The bracket `[ ]` is Char's core shape. It appears in:

- `CharLogo` — full (`[ char ]`) and compact (`[ ]`) variants
- The footer as large decorative SVG brackets flanking content

Brackets are always SVG. Never re-render as text in any font.

## Components

### Primary CTA

Warm stone gradient pill. Used in header, hero, CTA section.

- Heights vary by context: **32px** header/sidebar, **36px** standalone, **48px** page-level CTA.
- Gradient: `bg-linear-to-t from-stone-600 to-stone-500`.
- Micro-interaction: `hover:scale-[102%] hover:shadow-lg active:scale-[98%]`.

### Ghost / secondary

Outline, no fill. Used for secondary actions. `h-9, rounded-md, bg-white, border border-neutral-200, text-neutral-700, hover:bg-neutral-50`.

### Nav link

Text-only. Dotted underline on hover:

```
hover:underline hover:decoration-dotted hover:underline-offset-4
```

Muted by default; resolves to `fg` on hover.

### Section label

All-caps mono, wide tracking, muted. Uses `{typography.label-caps}`.

### Card

No heavy shadow. Border ring or hairline border, surface background. Two padding variants: `card` (compact, 16px) and `card-feature` (32px).

### Hero container

Bright-bordered rounded container at 88vh min-height. Only on the homepage hero.

### Hero mock (new)

The hero now contains an interactive "Daily Note" mock — a simulated app window pinned to the bottom of the notebook backdrop at `h-[90%]` of its container. It contains:

- **Tabs bar** — home (the daily-note view) plus dynamic tabs for opened meetings. Clicking a meeting row opens it in a new tab; the active tab uses `border-stone-400` + `bg-neutral-200/50`.
- **Scrollable day stack** — Tomorrow (header only), Today (content), Yesterday (content). `snap-y snap-proximity` with sticky headers. Today is the default scroll position; dates are computed live from `new Date()` with ordinal suffixes.
- **Meeting detail view** — Summary / Memos / Transcript tabs using the real `NoteTab` component from `@hypr/ui`. Summary uses the `.mock-summary` class for h4/ul formatting.
- **Avatar dropdown** — anchored top-right with keyboard-shortcut chips (Contacts ⌘⇧O, Calendar ⌘⇧C, Settings ⌘,, Help) and a user row with LITE badge.

### Logo

SVG, not type. Two variants via `CharLogo`:

| Variant | Description | Usage |
|---|---|---|
| Full | `[ char ]` wordmark | Desktop sidebar, tablet header |
| Compact | `[ ]` brackets only | Tablet header at smaller widths |

The footer carries its own SVG wordmark (just "char" letterforms, no brackets).

## Do's and Don'ts

### Do

- Leave whitespace. Char reads as calm — resist filling every pixel.
- Use the stone gradient for primary CTAs and `brand-yellow` only for the hero/footer wash.
- Keep h1/h2 in Geist Mono. Reach for serif only in editorial moments.
- Use `shadow-ring` in place of `border` when an element already has a box-shadow.
- Animate with restraint: `duration-200` or shorter, ease-in/out, no bounce.
- Render the logo as SVG. Always.

### Don't

- Introduce colors outside the palette. If you need one, update tokens first.
- Use `brand-yellow` on buttons or body text. It's a wash, never a fill.
- Stack two heavy weights adjacently — pair heavy with light.
- Use bounce/spring on brand UI.
- Re-render the bracket logo as text in any font.
- Add shadows to text. No inset shadows.

## Motion

| Pattern | Usage |
|---|---|
| `hover:scale-[102%] active:scale-[98%]` | Interactive cards, CTAs |
| `transition-opacity duration-200` | Fade in/out on dynamic text |
| `animate-in slide-in-from-top duration-300` | Mobile menu only |
| `opacity + y:-6 → 0`, 150ms ease-out | Avatar dropdown, flyouts |
| `motion/react` + `AnimatePresence` | Scroll-reveal panel CTA |

| Utility class | Description |
|---|---|
| `.animate-shake` | Horizontal shake — validation (0.5s) |
| `.animate-scroll-left`, `.animate-scroll-right` | Infinite logo-cloud scroll |
| `.animate-fade-in-out` | 3s fade loop (decorative) |
| `.animate-dot-wave` | 3s opacity wave (loading indicator) |

## Component folder structure

```
src/components/
  admin/           # Internal admin tooling
  mdx/             # MDX renderer overrides
  notepad/         # Notepad product feature demos
  sections/        # Composed page sections
  transcription/   # Transcription product feature demos
  *.tsx            # Flat root — layout, navigation, shared components
```

Most components currently live flat in `components/` (sidebar, footer, CTA section, download button, daily-note-mock, etc.). The `layout/` and `ui/` subdirectories from earlier plans haven't been created. When touching a file, consider moving it into the appropriate subfolder as part of that PR.
