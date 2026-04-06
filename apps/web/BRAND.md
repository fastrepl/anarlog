# Char Brand

This document is the single source of truth for Char's visual identity on the web. All tokens referenced here are defined in `src/styles.css` and available as Tailwind utilities.

---

## Color

The palette is warm neutral — rooted in stone and off-white. One moment of warmth (the brand gradient on CTAs). Everything else recedes.

### Palette

| Role | Token | Hex | Tailwind class |
|---|---|---|---|
| Page background | `--color-page` | `#F2F3F4` | `bg-page` |
| Surface | `--color-surface` | `#ffffff` | `bg-surface` |
| Surface muted | `--color-surface-subtle` | `#fafaf9` | `bg-surface-subtle` |
| Primary text | `--color-fg` | `#1c1917` | `text-fg` |
| Secondary text | `--color-fg-muted` | `#57534e` | `text-fg-muted` |
| Placeholder / disabled | `--color-fg-subtle` | `#a8a29e` | `text-fg-subtle` |
| Default border | `--color-border` | `#CBC8BD` | `border-border` |
| Hairline border | `--color-border-subtle` | `#f5f5f5` | `border-border-subtle` |
| Brand accent | `--color-brand` | `#78716c` | `bg-brand` / `text-brand` |
| Brand dark | `--color-brand-dark` | `#57534e` | `bg-brand-dark` |

### Usage rules

- Never introduce a color outside this palette without updating the token set first.
- `color-brand` and `color-brand-dark` are the only "warm accent" moments. Use them exclusively for primary CTAs and interactive focus states.
- Tailwind's `neutral-*` and `stone-*` scales are available as fallback but semantic tokens above take precedence for any brand-facing UI.

---

## Typography

Three typefaces, each with a distinct role. Do not mix roles.

| Face | Font | Variable | Role |
|---|---|---|---|
| Serif | Fraunces | `--font-serif` | Wordmark, editorial pull-quotes |
| Sans | Geist | `--font-sans` | All body copy, UI labels, navigation |
| Mono | Geist Mono | `--font-mono` | Button labels, display headings, code, technical UI |

### Type scale principles

- **Heading hierarchy**: h1/h2 use `font-mono` by default (per base styles). Serif is used selectively for editorial or brand moments, not for all headings.
- **Weight contrast**: Pair heavy display weight (`font-semibold` / `font-bold`) with light body weight (`font-normal`). Never use two heavy weights adjacently.
- **Letter spacing**: Tight (`tracking-tight`) on large display type. Open (`tracking-wider`) on all-caps labels and category tags.
- **Minimum readable size**: 14px for body, 12px only for all-caps labels or metadata.

---

## Borders & Shadows

| Token | Value | Class | Use |
|---|---|---|---|
| `--shadow-ring` | `0 0 0 1px #CBC8BD` | `.border-shadow` | Default card/panel outline |
| `--shadow-ring-subtle` | `0 0 0 1px #f5f5f5` | — | Hairline structural outlines |

**Prefer `shadow-ring` over CSS `border`** when an element already has box-shadow — avoids double-border stacking issues.

Border radius conventions:
- `rounded-xs` — tight UI elements (chips, small badges)
- `rounded-md` — cards, inputs, dropdowns
- `rounded-lg` — modals, large cards
- `rounded-full` — pill buttons, avatars, tags

---

## Spacing

The layout uses a base-4 spacing rhythm. Key structural values:

| Purpose | Value |
|---|---|
| Header height | `69px` / `h-17.25` |
| Content max-width | `max-w-6xl` |
| Wide breakpoint | `72rem` (`laptop`) |
| Section vertical padding | `py-12` (mobile) / `py-16` (desktop) |
| Card internal padding | `p-4` (compact) / `p-8` (feature) |

---

## Components

### Primary CTA button

The main action. Warm gradient, pill shape, scales on hover.

```tsx
<button className="flex h-9 items-center rounded-full bg-linear-to-t from-brand-dark to-brand px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]">
  Download for free
</button>
```

### Secondary / ghost button

Outline style, no fill. Used for secondary actions.

```tsx
<button className="flex h-9 items-center rounded-lg border border-border bg-surface px-4 text-sm text-fg-muted transition-colors hover:bg-surface-subtle">
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

Or with `shadow-ring` instead of `border`:

```tsx
<div className="border-shadow rounded-md bg-surface p-4">
  …
</div>
```

---

## Logo

- **Wordmark**: "Char" in `font-serif` (Fraunces), `text-2xl`, `font-semibold`.
- **Do not** render the wordmark in `font-sans` or `font-mono`.
- **Scale animation** on hover: `hover:scale-105 transition-transform` is the only permitted motion on the logo.

---

## Motion

- Scale micro-interactions: `hover:scale-[102%] active:scale-[98%]` — used on all interactive cards and CTA buttons.
- Opacity transitions: `transition-opacity duration-200` — used for fade in/out on dynamic text.
- Page-level slide-in: `animate-in slide-in-from-top duration-300` — used for mobile menu only.
- No bounce, no spring, no decorative keyframes on brand UI.

---

## Component folder structure

```
src/components/
  layout/         # Header, Footer, Sidebar — structural chrome
  ui/             # Primitive, stateless brand components (Button, Badge, Card)
  sections/       # Composed page sections (LogoCloud, CtaCard, GithubStars)
  mdx/            # MDX renderer overrides
  notepad/        # Notepad product feature
  transcription/  # Transcription product feature
  admin/          # Internal admin tooling
```

When creating a new component, ask:
- Is it a stateless visual primitive? → `ui/`
- Is it a full page section? → `sections/`
- Is it structural layout chrome? → `layout/`
- Is it tied to a specific product feature? → that feature's folder
