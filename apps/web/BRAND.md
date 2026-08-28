# Anarlog Brand

This document is the source of truth for Anarlog's web identity. Component
styles are written with StyleX. Shared constants come from
`@anlg/design-system/tokens.stylex`; web-only global variables and browser
resets live in `src/styles.css`.

## Styling contract

- Use `stylex.create` and `stylex.props` for component styling.
- Prefer `colors`, `fonts`, `media`, `radii`, `shadows`, and `spacing` from the
  shared StyleX token module.
- Use `mergeStyleXProps` when a component must combine StyleX with a third-party
  `className` or inline `style` prop.
- Keep global CSS limited to browser resets and root-level variables.
- Do not add utility-class styling, generated `--tw-*` properties, or numeric CSS
  values encoded as strings when StyleX expects numbers.
- Define animations with `stylex.keyframes` and animation longhands.

## Visual direction

The marketing site should feel like a carefully crafted notebook in a digital
space: restrained grids and lines, handwritten details beside structured
interfaces, generous whitespace, and few distracting colors. The product
should feel calm, local, and deliberate.

## Color

The palette is warm neutral. Brand yellow is reserved for broad atmospheric
washes; interactive emphasis uses dark stone tones.

### Foundation

| Token        | Value                         |
| ------------ | ----------------------------- |
| `--grey-900` | `oklch(0.3 0.0197 81.53)`     |
| `--grey-700` | `oklch(0.4922 0.0127 67.79)`  |
| `--grey-600` | `oklch(0.6182 0.0018 67.8)`   |
| `--grey-500` | `oklch(0.7782 0.0018 67.8)`   |
| `--grey-300` | `oklch(0.9213 0.0027 106.45)` |
| `--grey-100` | `oklch(0.9558 0.0045 78.3)`   |

### Semantic palette

| Role            | Web token                  | StyleX equivalent             |
| --------------- | -------------------------- | ----------------------------- |
| Page background | `--color-page`             | `colors.background`           |
| Surface         | `--color-surface`          | `colors.card`                 |
| Muted surface   | `--color-surface-subtle`   | `colors.muted`                |
| Primary text    | `--color-fg`               | `colors.foreground`           |
| Secondary text  | `--color-fg-muted`         | `colors.mutedForeground`      |
| Default border  | `--color-border`           | `colors.border`               |
| Accent border   | `--color-border-bright`    | Use the existing web variable |
| Dark emphasis   | `--color-brand-dark`       | `colors.primary`              |
| Warm wash       | `--brand-yellow`           | Use the existing web variable |

Do not introduce a color outside this palette without first adding an
intentional semantic token. Brand yellow is not a button or text color.

## Typography

| Constant          | Face                                  | Role                              |
| ----------------- | ------------------------------------- | --------------------------------- |
| `fonts.sans`      | System UI / Segoe UI fallback stack   | Body, navigation, and UI labels   |
| `fonts.mono`      | System monospace fallback stack       | Code and compact technical labels |
| `fonts.hand`      | Caveat / handwritten fallback stack   | Editorial and marketing headings  |
| `--font-signature`| Patrick Hand                           | Signature-like accents            |

Use numeric `fontWeight` and unitless numeric `lineHeight` values in StyleX.
Large marketing headings can use `fonts.hand`; product surfaces should default
to `fonts.sans`.

Minimum readable body text is 14px. Reserve 12px for uppercase labels or
metadata, and pair heavier headings with normal-weight body copy.

## Borders, radii, and shadows

Use `radii.sm`, `radii.md`, `radii.lg`, `radii.xl`, and `radii.full` rather than
large generated radius values. A deliberate 3px paper-card radius is acceptable
where the visual calls for a physical sheet rather than a software panel.

`--shadow-ring` and `--shadow-ring-left` remain available for web-only outline
effects. Prefer semantic `shadows.sm` or `shadows.lg` for product surfaces.

## Layout

The marketing site uses a centered content column and expands into composed
sections at larger widths.

| Constant   | Value  | Purpose                     |
| ---------- | ------ | --------------------------- |
| `media.sm` | 40rem  | Compact-to-tablet changes   |
| `media.md` | 48rem  | Tablet and desktop changes  |
| Wide tier  | 80rem  | Shared-note comment rail    |
| `laptop`   | 72rem  | Legacy web layout threshold |
| `wide`     | 87.5rem| Extra-wide layout threshold |

Keep responsive conditions in StyleX property maps. Use DOM order for local
overlap and portals for cross-tree floating UI; add `zIndex` only inside a
bounded stacking context.

## Component patterns

### Primary action

```tsx
const styles = stylex.create({
  primaryAction: {
    alignItems: "center",
    backgroundColor: {
      default: colors.primary,
      ":hover": colors.secondaryForeground,
    },
    borderRadius: radii.full,
    color: colors.primaryForeground,
    display: "inline-flex",
    minHeight: "2.75rem",
    paddingInline: "1.25rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color, transform",
  },
});
```

### Secondary action

Use `colors.card`, `colors.border`, and `radii.lg`. Hover states should change
one semantic surface or foreground token, not introduce a new color.

### Navigation link

Use muted foreground by default, primary foreground on hover, and a dotted
underline when extra affordance is needed.

### Card

Cards use a semantic surface, a quiet one-pixel border, and at most a small
shadow. Feature cards may use a paper texture or notebook line pattern defined
directly in their StyleX style.

### Section label

Section labels are small, wide-tracked, and restrained. Use `fonts.mono`, a
numeric semibold weight, uppercase text, and `colors.mutedForeground`.

## Background treatments

Marketing pages may use two subtle layers:

1. A gradient from `var(--brand-yellow)` to transparent.
2. The generated noise texture from `src/lib/brand-noise.ts`, masked so it fades
   into the page.

Resource pages such as docs, blog, gallery, and changelog omit the yellow wash.
Decorative notebook, dot, and grid patterns should be expressed in local StyleX
`backgroundImage` declarations, not global utility classes.

## Document content

Blog and legal MDX use contextual selectors on their article StyleX styles.
Changelog content accepts an `sx` prop and should be styled through that
component API.

## Logo

Use the official black wordmark from `public/logo.svg` through the
`AnarlogLogo` component. Preserve its aspect ratio and use the accessible name
“Anarlog” when the image is meaningful. Do not recreate or alter the wordmark.

## Motion

- Use restrained opacity, transform, and color transitions.
- Define keyframes with `stylex.keyframes`.
- Set `animationName`, `animationDuration`, `animationTimingFunction`, and
  `animationIterationCount` separately.
- Disable non-essential motion under `media.reducedMotion`.
- Avoid bounce and decorative spring motion in brand UI.

## Component structure

```text
src/components/
  admin/           # Internal admin tooling
  mdx/             # MDX renderer overrides
  notepad/         # Notepad product demos
  sections/        # Composed marketing sections
  transcription/  # Transcription product demos
  *.tsx            # Layout, navigation, and shared components
```
