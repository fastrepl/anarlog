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

| Token | Value | Use for |
|---|---|---|
| `--color-page` | `#F2F3F4` | Page/canvas background (`bg-page`) |
| `--color-surface` | `#ffffff` | Card, panel, modal backgrounds (`bg-surface`) |
| `--color-surface-subtle` | `#fafaf9` | Muted surface variants (`bg-surface-subtle`) |
| `--color-fg` | `#1c1917` | Primary text (`text-fg`) |
| `--color-fg-muted` | `#57534e` | Secondary/body text (`text-fg-muted`) |
| `--color-fg-subtle` | `#a8a29e` | Placeholder, disabled, icons (`text-fg-subtle`) |
| `--color-border` | `#CBC8BD` | Default borders, dividers (`border-border`) |
| `--color-border-subtle` | `#f5f5f5` | Hairline/structural borders (`border-border-subtle`) |
| `--color-brand` | `#78716c` | Brand accent — use sparingly (`bg-brand`, `text-brand`) |
| `--color-brand-dark` | `#57534e` | CTA button gradient end, emphasis (`bg-brand-dark`) |

### Shadow tokens

| Token | Use for |
|---|---|
| `--shadow-ring` | 1px outline border effect — prefer over `border` when stacking borders |
| `--shadow-ring-subtle` | Same, using the subtle border color |

The `.border-shadow` utility class applies `--shadow-ring` as a convenience.

### Typography

- **Display / logo**: `font-serif` (Fraunces) — headings, logo wordmark, editorial emphasis
- **Body / UI text**: `font-sans` (Geist) — all body copy, labels, nav links
- **Code / buttons**: `font-mono` (Geist Mono) — code, button labels, monospaced UI

### CTA button pattern

Primary CTA always uses the warm gradient:

```tsx
"bg-linear-to-t from-brand-dark to-brand rounded-full text-white"
```

Secondary / ghost uses surface + border:

```tsx
"bg-surface border border-border rounded-lg text-fg-muted"
```

## Component structure

Target folder layout. New components must go in the right folder — do not add to the flat root of `components/`.

```
src/components/
  layout/         # Header, Footer, Sidebar, SidebarNavigation
  ui/             # Primitive brand components: Button, Badge, Card shells
  sections/       # Page-level marketing sections (LogoCloud, CtaCard, GithubStars…)
  mdx/            # MDX renderer components
  notepad/        # Notepad feature
  transcription/  # Transcription feature
  admin/          # Admin tooling
```

Existing flat-root files are being migrated incrementally. When touching a file, move it to the right folder as part of that PR.

For full brand reference, see `BRAND.md`.
