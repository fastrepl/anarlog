import * as stylex from "@stylexjs/stylex";
import type { ComponentType } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

const styles = stylex.create({
  style1: {
    marginBlock: "1.5rem",
    width: "100%",
    borderRadius: ".375rem",
  },
  style2: {
    marginBlock: "1.5rem",
    display: "block",
    borderRadius: ".375rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: {
      default: "#e5e5e5",
      ":hover": "#a8a29e",
    },
    padding: "1.5rem",
    textDecorationLine: "none",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    backgroundColor: {
      default: null,
      ":hover": "#fafaf9",
    },
  },
  style3: {
    marginBottom: ".25rem",
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, Courier New, monospace",
    fontSize: "1rem",
    lineHeight: "1.5rem",
    color: "#292524",
  },
  style4: {
    marginBottom: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#525252",
  },
  style5: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#57534e",
  },
  style6: {
    marginBlock: "1.5rem",
    borderRadius: ".375rem",
    borderStyle: "solid",
    borderWidth: "1px",
    padding: "1rem",
  },
  style7: {
    marginBlock: "1.5rem",
    width: "100%",
    overflow: "hidden",
    borderRadius: ".375rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#e5e5e5",
  },
  style8: {
    height: "100%",
    width: "100%",
  },
  style9: {
    borderRadius: radii.sm,
    backgroundColor: colors.muted,
    paddingInline: ".375rem",
    paddingBlock: ".125rem",
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: colors.foreground,
  },
  calloutNote: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
  },
  calloutTip: {
    backgroundColor: "oklch(96.2% 0.044 156.7)",
    borderColor: "oklch(87.1% 0.15 154.4)",
  },
  calloutWarning: {
    backgroundColor: "oklch(98.7% 0.022 95.3)",
    borderColor: "oklch(87.9% 0.169 91.6)",
  },
});
function Image({
  src,
  alt,
  className,
  style,
  ...rest
}: {
  src: string;
  alt?: string;
  [k: string]: any;
}) {
  return (
    <img
      {...rest}
      {...mergeStyleXProps(styles.style1, className, style)}
      src={src}
      alt={alt ?? ""}
    />
  );
}
function CtaCard({
  href,
  title,
  description,
  cta,
}: {
  href?: string;
  title?: string;
  description?: string;
  cta?: string;
}) {
  if (!href) return null;
  return (
    <a href={href} {...stylex.props(styles.style2)}>
      {title && <div {...stylex.props(styles.style3)}>{title}</div>}
      {description && <div {...stylex.props(styles.style4)}>{description}</div>}
      {cta && <div {...stylex.props(styles.style5)}>{cta} →</div>}
    </a>
  );
}
function Callout({
  type = "note",
  children,
}: {
  type?: string;
  children?: React.ReactNode;
}) {
  const tone =
    type === "warning"
      ? styles.calloutWarning
      : type === "tip"
        ? styles.calloutTip
        : styles.calloutNote;
  return <aside {...stylex.props(styles.style6, tone)}>{children}</aside>;
}
function Clip({ src }: { src: string }) {
  const ytMatch = src.match(/(?:youtube\.com\/watch\?v=|youtu\.be\/)([^&]+)/);
  if (ytMatch) {
    return (
      <div {...stylex.props(styles.style7)}>
        <iframe
          src={`https://www.youtube.com/embed/${ytMatch[1]}`}
          {...stylex.props(styles.style8)}
          allowFullScreen
        />
      </div>
    );
  }
  return null;
}
const Noop = () => null;
function InlineCode({
  children,
  className,
  style,
  ...props
}: React.ComponentProps<"code">) {
  return (
    <code {...props} {...mergeStyleXProps(styles.style9, className, style)}>
      {children}
    </code>
  );
}
export const mdxComponents: Record<string, ComponentType<any>> = {
  Image,
  img: Image,
  CtaCard,
  Callout,
  Clip,
  Aside: Noop,
  Figure: Noop,
  CodeBlock: Noop,
  ComparisonTable: Noop,
  Grid: Noop,
  Tabs: Noop,
  Video: Noop,
  code: InlineCode,
};
