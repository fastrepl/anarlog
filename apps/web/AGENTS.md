```bash
infisical export \
  --env=dev \
  --secret-overriding=false \
  --format=dotenv \
  --output-file="apps/web/.env" \
  --projectId=87dad7b5-72a6-4791-9228-b3b86b169db1 \
  --path="/web"
```

## Design System

All visual tokens live in `src/styles.css` inside the `@theme` block. Never use hardcoded hex values in components — always reference a token.

### Color tokens

The palette is built on an oklch grey scale. Key semantic tokens:

| Token | Value | Use for |
|---|---|---|
| `--color-page` | `#f2f1ef` | Page/canvas background (`bg-page`) |
| `--color-surface` | `#ffffff` | Card, panel, modal backgrounds (`.surface`) |
| `--color-surface-subtle` | `var(--grey-100)` | Muted surface variants (`.surface-subtle`) |
| `--color-fg` | `var(--grey-900)` | Primary text (`.text-color`) |
| `--color-fg-muted` | `#57534e` | Secondary/body text (`.text-color-muted`) |
| `--color-fg-subtle` | `var(--color-border)` | Placeholder, disabled, icons |
| `--color-border` | `var(--grey-500)` | Default borders (`.border-color-brand`) |
| `--color-border-subtle` | `var(--grey-300)` | Hairline/structural borders (`.border-color-subtle`) |
| `--color-border-bright` | `oklch(0.5959 0.0333 78.6)` | Accent borders (`.border-color-bright`) |
| `--color-brand-dark` | `#57534e` | Checked states, emphasis (`bg-brand-dark`) |
| `--brand-yellow` | `oklch(0.9484 0.0672 90.6)` | Hero/footer warm wash (`.brand-yellow`) |

Note: there is no `--color-brand` token. CTAs use `from-stone-600 to-stone-500`.

### Shadow tokens

| Token | Use for |
|---|---|
| `--shadow-ring` | 1px outline border effect (`.border-around`) — prefer over `border` when stacking |
| `--shadow-ring-left` | Left-edge only 1px outline |

### Typography

- **Body / UI text**: `font-sans` (Geist) — all body copy, labels, nav links
- **Headings / buttons**: `font-mono` (Geist Mono) — h1/h2, button labels, code
- **Editorial**: `font-serif` (Fraunces) — editorial emphasis, decorative moments
- **Italic accent**: `font-serif2` (Instrument Serif) — italic editorial accents

### CTA button pattern

Primary CTA uses Tailwind's stone gradient (not a custom token):

```tsx
"bg-linear-to-t from-stone-600 to-stone-500 rounded-full text-white"
```

Secondary / ghost uses surface + border:

```tsx
"border border-neutral-200 bg-white rounded-lg text-neutral-700"
```

## Component structure

Current folder layout:

```
src/components/
  admin/           # Internal admin tooling
  mdx/             # MDX renderer components
  notepad/         # Notepad feature demos
  sections/        # Page-level marketing sections
  transcription/   # Transcription feature demos
  *.tsx            # Flat root — layout, navigation, shared components
```

Most components currently live flat in the root. When touching a file, consider moving it to the appropriate subfolder.

For full brand reference, see `BRAND.md`.
