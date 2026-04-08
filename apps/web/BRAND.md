# Char Brand

This document is the single source of truth for Char's visual identity on the web. All tokens referenced here are defined in `src/styles.css` and available as Tailwind utilities.

---

## Review
Overall feeling from the marketing websites should be like from carefully crafted notebook that lives in the digital space: lines and grids from the notebook with handwritten patterns or dropped graphic on it (like cut from other sources) stays near the mono fonts, structured outputs and interfaces.
Char is brand that values place for thoughts and slow pace of exploration. It transforms into layouts with a lot of air in them and free spaces. Less distracting colors and small elements.

## Color

The palette is warm neutral — built on an oklch grey scale with off-white and stone tones. The one moment of warmth is the brand yellow used for hero gradients and the footer. CTAs use a stone gradient. Everything else recedes.

### Grey scale (foundation)

These oklch values power most semantic tokens through `var()` references.

| Token | Value |
|---|---|
| `--grey-900` | `oklch(0.3 0.0197 81.53)` |
| `--grey-700` | `oklch(0.4922 0.0127 67.79)` |
| `--grey-500` | `oklch(0.7782 0.0018 67.8)` |
| `--grey-300` | `oklch(0.9213 0.0027 106.45)` |
| `--grey-100` | `oklch(0.9558 0.0045 78.3)` |

### Semantic palette

| Role | Token | Value | Tailwind / utility class |
|---|---|---|---|
| Page background | `--color-page` | `#f2f1ef` | `bg-page` |
| Surface | `--color-surface` | `#ffffff` | `bg-surface`, `.surface` |
| Surface muted | `--color-surface-subtle` | `var(--grey-100)` | `bg-surface-subtle`, `.surface-subtle` |
| Primary text | `--color-fg` | `var(--grey-900)` | `text-fg`, `.text-color` |
| Secondary text | `--color-fg-muted` | `#57534e` | `text-fg-muted`, `.text-color-muted` |
| Placeholder / disabled | `--color-fg-subtle` | `var(--color-border)` | `text-fg-subtle` |
| Default border | `--color-border` | `var(--grey-500)` | `.border-color-brand`, `.divide-brand` |
| Hairline border | `--color-border-subtle` | `var(--grey-300)` | `.border-color-subtle` |
| Accent border | `--color-border-bright` | `oklch(0.5959 0.0333 78.6)` | `.border-color-bright` |
| Active border | `--color-border-active` | `oklch(0.9213 0.0027 106.45)` | — |
| Brand dark | `--color-brand-dark` | `#57534e` | `bg-brand-dark` |
| Brand yellow | `--brand-yellow` | `oklch(0.9484 0.0672 90.6)` | `.brand-yellow` |

### Usage rules

- Never introduce a color outside this palette without updating the token set first.
- `brand-dark` is used for checked states and emphasis. CTAs use `from-stone-600 to-stone-500` (Tailwind's stone scale), not a custom token gradient.
- `brand-yellow` is the warm wash at the top of marketing pages and the footer gradient. It is **not** used for buttons or text.
- Tailwind's `neutral-*` and `stone-*` scales appear in components as fallback, but semantic tokens above take precedence for brand-facing UI.
- `--color-fg-subtle` deliberately aliases `--color-border` so that placeholder text and borders share the same value.

---

## Typography

Six typefaces are loaded; the first three carry distinct roles. The rest are special-purpose.

### Primary stack

| Face | Font | Variable | Role |
|---|---|---|---|
| Serif | Fraunces | `--font-serif` | Wordmark weight, editorial pull-quotes |
| Sans | Geist | `--font-sans` | All body copy, UI labels, navigation |
| Mono | Geist Mono | `--font-mono` | Button labels, display headings (h1/h2), code |

### Secondary / special-purpose

| Face | Font | Variable | Role |
|---|---|---|---|
| Serif 2 | Instrument Serif | `--font-serif2` | Italic editorial accents |
| Display | Redaction | (local, `@font-face`) | Decorative / redacted text effects |
| System | SF Pro | (local, `@font-face`) | System-matching UI contexts |
| Serif (alt) | Lora | (Google Fonts import) | Loaded but sparingly used |

### Base layer rules (from `styles.css`)

- `html, body` → `font-sans`
- `h1, h2` → `font-mono`, weight 500, tracking -0.02em, line-height 1.3
- `h3–h6, p, span, li` → `font-sans`
- `p` → `font-size: 1.3rem`, `line-height: 1.5`
- `button, [role="button"]` → `font-mono`

### Type scale principles

- **Heading hierarchy**: h1/h2 use `font-mono` by default (per base styles). Serif is used selectively for editorial or brand moments, not for all headings.
- **Weight contrast**: Pair heavy display weight (`font-semibold` / `font-bold`) with light body weight (`font-normal`). Never use two heavy weights adjacently.
- **Letter spacing**: Tight (`tracking-tight`) on large display type. Open (`tracking-wider`) on all-caps labels and category tags.
- **Minimum readable size**: 14px for body, 12px only for all-caps labels or metadata.

---

## Borders & Shadows

| Token | Value | Class | Use |
|---|---|---|---|
| `--shadow-ring` | `0 0 0 1px var(--color-border)` | `.border-around` | Default card/panel outline |
| `--shadow-ring-left` | `-1px 0 0 1px var(--color-border)` | — | Left-edge only outline |

**Prefer `shadow-ring` over CSS `border`** when an element already has box-shadow — avoids double-border stacking issues.

Border radius conventions:
- `rounded-xs` — tight UI elements (small badges, dropdown panels)
- `rounded-md` — cards, inputs, dropdowns
- `rounded-lg` — modals, large cards, hero containers
- `rounded-full` — pill buttons, avatars, tags

---

## Layout

The marketing site uses a 3-column layout on large screens: left Sidebar, center content, right RightPanel.

### Breakpoints

| Name | Value | Purpose |
|---|---|---|
| `laptop` | `72rem` (1152px) | General responsive breakpoint |
| `wide` | `87.5rem` (1400px) | Wide sidebar/panel sizing |
| `xl` (Tailwind) | `1280px` | Desktop sidebar appears |
| `md` (Tailwind) | `768px` | Tablet header bar appears |

### Key dimensions

| Purpose | Value |
|---|---|
| Outer max-width | `max-w-[1800px]` (3-column wrapper) |
| Content max-width | `max-w-6xl` (footer, header bar) |
| Mobile top bar height | `h-14` (56px) |
| Scroll margin (anchors) | `69px` |
| Section vertical padding | `py-12` (mobile) / `py-16` (desktop) |
| Card internal padding | `p-4` (compact) / `p-8` (feature) |

### Responsive layout tiers

| Range | Layout |
|---|---|
| `< md` (< 768px) | Fixed top bar + hamburger dropdown, single column |
| `md – xl` (768–1280px) | Fixed horizontal header bar, single column |
| `xl+` (1280px+) | Sticky left Sidebar + content + sticky RightPanel |

---

## Brand yellow & noise

Marketing pages (homepage, product pages, etc.) have a warm yellow gradient wash at the top, rendered as two overlapping layers:

1. A CSS gradient from `var(--brand-yellow)` to `transparent` covering `h-[180vh]`.
2. A repeating noise texture at 30% opacity with a mask that fades it out downward. The noise is generated from `src/lib/brand-noise.ts`.

Resource pages (docs, blog, gallery, changelog, etc.) skip this background.

The footer mirrors the effect in reverse: a gradient from `transparent` to `var(--brand-yellow)` with the same noise texture.

---

## Background patterns

Utility classes for decorative backgrounds applied to sections and cards:

| Class | Pattern |
|---|---|
| `.bg-lined-notebook` | Horizontal lines (subtle border color) |
| `.bg-lined-notebook-dark` | Horizontal lines (default border color) |
| `.bg-dotted` | Dot grid (subtle) |
| `.bg-dotted-dark` | Dot grid (default) |
| `.bg-grid` | Full grid (subtle) |
| `.bg-grid-dark` | Full grid (default) |

All patterns use 24px spacing (23px gap + 1px line) and reference border color tokens.

---

## Components

### Primary CTA button

Warm stone gradient, pill shape, scales on hover. Used in header, hero, and CTA sections.

```tsx
<button className={cn([
  "flex h-8 items-center rounded-full px-4 text-sm text-white",
  "bg-linear-to-t from-stone-600 to-stone-500",
  "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
  "transition-all",
])}>
  Download for free
</button>
```

Heights vary by context: `h-8` in header/sidebar, `h-9` in standalone, `h-12` in page-level CTA sections.

### Secondary / ghost button

Outline style, no fill. Used for secondary actions.

```tsx
<button className="flex h-9 items-center rounded-lg border border-neutral-200 bg-white px-4 text-sm text-neutral-700 transition-colors hover:bg-neutral-50">
  Get started
</button>
```

### Nav link

Text-only, dotted underline on hover.

```tsx
<a className="text-sm text-fg-muted decoration-dotted transition-colors hover:text-fg hover:underline">
  Link
</a>
```

The standard hover class used across navigation is:
```tsx
const MAIN_MENU_LINK_HOVER = "hover:underline hover:decoration-dotted hover:underline-offset-4";
```

### Section label (category tag)

All-caps mono, wide tracking, muted.

```tsx
<span className="font-mono text-xs font-semibold tracking-wider text-fg-subtle uppercase">
  Features
</span>
```

### Card

No heavy shadow. Border ring or hairline border, surface background.

```tsx
<div className="rounded-md border border-border bg-surface p-4">
  …
</div>
```

Or with `shadow-ring`:

```tsx
<div className="border-around rounded-md bg-surface p-4">
  …
</div>
```

### Hero container

The homepage hero uses a bright-bordered rounded container at full viewport height:

```tsx
<div className="border-brand-bright rounded-lg border min-h-[80vh]">
  …
</div>
```

---

## Logo

The logo is an **SVG**, not text rendered in a font. It comes in two variants via the `CharLogo` component (`src/components/sidebar.tsx`):

| Variant | Description | Usage |
|---|---|---|
| Full | "char" wordmark flanked by bracket marks `[ char ]` | Desktop sidebar, tablet header |
| Compact | Bracket marks only `[ ]` | Tablet header at smaller widths (< lg) |

The footer has its own SVG wordmark (just "char" letterforms, no brackets).

- **Scale animation** on hover: `hover:scale-105 transition-colors` on the sidebar logo.
- **Do not** re-render the logo as text in any font. Always use the SVG component.

### Bracket motif

The bracket `[ ]` shape is a core visual element. It appears in:
- The `CharLogo` component (full and compact variants)
- The footer as large decorative SVG brackets on either side

---

## Motion

- **Scale micro-interactions**: `hover:scale-[102%] active:scale-[98%]` — used on all interactive cards and CTA buttons.
- **Opacity transitions**: `transition-opacity duration-200` — used for fade in/out on dynamic text.
- **Page-level slide-in**: `animate-in slide-in-from-top duration-300` — used for mobile menu only.
- **Scroll reveal**: RightPanel CTA fades in after scrolling past the viewport height (motion/react `AnimatePresence`).
- **Flyout menus**: sidebar flyouts use `opacity + x` transitions (`duration: 0.15, ease: easeInOut`).
- No bounce, no spring, no decorative keyframes on brand UI.

### Animation utility classes

| Class | Description |
|---|---|
| `.animate-shake` | Horizontal shake (validation feedback, 0.5s) |
| `.animate-scroll-left` | Infinite horizontal scroll left (logo clouds) |
| `.animate-scroll-right` | Infinite horizontal scroll right |
| `.animate-fade-in-out` | 3s fade in/out loop (decorative) |
| `.animate-dot-wave` | 3s opacity wave (loading indicators) |

---

## Component folder structure

```
src/components/
  admin/           # Internal admin tooling
  mdx/             # MDX renderer overrides
  notepad/         # Notepad product feature demos
  sections/        # Composed page sections
  transcription/   # Transcription product feature demos
  *.tsx            # Flat root — layout, navigation, and shared components
```

Most components currently live flat in the `components/` root (sidebar, footer, CTA section, download button, etc.). The `layout/` and `ui/` subdirectories described in earlier plans have not been created yet. When touching a file, consider moving it to the appropriate subfolder as part of that PR.
