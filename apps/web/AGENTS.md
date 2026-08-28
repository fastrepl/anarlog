```bash
infisical export \
  --env=dev \
  --secret-overriding=false \
  --format=dotenv \
  --output-file="apps/web/.env" \
  --projectId=87dad7b5-72a6-4791-9228-b3b86b169db1 \
  --path="/anarlog/web"
```

## Design system

Component styles use `@stylexjs/stylex`. Reuse constants from
`@anlg/design-system/tokens.stylex` and the web CSS variables in
`src/styles.css`; do not introduce utility-class styling or new hardcoded
colors.

### Color tokens

The palette is built on an oklch grey scale. Key semantic tokens:

| Token                    | Value                       | Use for                       |
| ------------------------ | --------------------------- | ----------------------------- |
| `--color-page`           | `#f2f1ef`                   | Page/canvas background        |
| `--color-surface`        | `#ffffff`                   | Card, panel, modal background |
| `--color-surface-subtle` | `var(--grey-100)`           | Muted surfaces                |
| `--color-fg`             | `var(--grey-900)`           | Primary text                  |
| `--color-fg-muted`       | `var(--grey-500)`           | Secondary text                |
| `--color-fg-secondary`   | `var(--grey-600)`           | Supporting text               |
| `--color-border`         | `var(--grey-500)`           | Default borders               |
| `--color-border-subtle`  | `var(--grey-300)`           | Hairlines                     |
| `--color-border-bright`  | `oklch(0.5959 0.0333 78.6)` | Accent borders                |
| `--color-brand-dark`     | `#57534e`                   | Checked states and emphasis   |
| `--brand-yellow`         | `oklch(0.9484 0.0672 90.6)` | Hero/footer warm wash         |

There is no `--color-brand` token.

### Shadow tokens

| Token                | Use for                    |
| -------------------- | -------------------------- |
| `--shadow-ring`      | 1px outline border effect |
| `--shadow-ring-left` | Left-edge 1px outline      |

### Typography

- Body and UI: `fonts.sans`
- Code and compact technical labels: `fonts.mono`
- Handwritten editorial headings: `fonts.hand`

### CTA button pattern

Primary CTAs use StyleX with a stone gradient, a full radius token, and explicit
interaction styles:

```tsx
const styles = stylex.create({
  primaryCta: {
    backgroundImage: "linear-gradient(to top, #57534e, #78716c)",
    borderRadius: radii.full,
    color: colors.primaryForeground,
  },
});
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
