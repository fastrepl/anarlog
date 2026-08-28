import * as stylex from "@stylexjs/stylex";
import type { ComponentProps, ComponentType } from "react";

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
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    backgroundColor: {
      default: null,
      ":hover": "#fafaf9",
    },
  },
  style3: {
    marginBottom: ".25rem",
    fontFamily: fonts.mono,
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
    overflowWrap: "anywhere",
    wordBreak: "break-all",
  },
  proseLink: {
    color: {
      default: "#181613",
      ":hover": "#4f4940",
    },
    textDecorationLine: "underline",
  },
  headingAnchor: {
    color: "inherit",
    textDecorationLine: {
      default: "none",
      ":hover": "underline",
    },
    textDecorationStyle: {
      default: "solid",
      ":hover": "dotted",
    },
  },
  taskList: {
    listStyleType: "none",
    marginLeft: 0,
    paddingLeft: 0,
  },
  taskListItem: {
    alignItems: "baseline",
    display: "flex",
    gap: ".5rem",
    marginLeft: 0,
    paddingLeft: 0,
  },
  taskCheckbox: {
    accentColor: colors.primary,
    appearance: "none",
    backgroundColor: {
      default: "transparent",
      ":checked": colors.primary,
    },
    borderColor: {
      default: colors.border,
      ":checked": colors.primary,
    },
    borderRadius: ".25rem",
    borderStyle: "solid",
    borderWidth: "1.5px",
    cursor: "pointer",
    margin: 0,
    minWidth: "1rem",
    borderBottomColor: {
      default: null,
      ":checked::after": colors.primaryForeground,
    },
    borderBottomStyle: {
      default: null,
      ":checked::after": "solid",
    },
    borderBottomWidth: {
      default: null,
      ":checked::after": "2px",
    },
    borderRightColor: {
      default: null,
      ":checked::after": colors.primaryForeground,
    },
    borderRightStyle: {
      default: null,
      ":checked::after": "solid",
    },
    borderRightWidth: {
      default: null,
      ":checked::after": "2px",
    },
    content: {
      default: null,
      ":checked::after": '""',
    },
    height: {
      default: "1rem",
      ":checked::after": ".6rem",
    },
    left: {
      default: null,
      ":checked::after": ".2rem",
    },
    position: {
      default: "relative",
      ":checked::after": "absolute",
    },
    top: {
      default: ".125rem",
      ":checked::after": ".05rem",
    },
    transform: {
      default: null,
      ":checked::after": "rotate(45deg)",
    },
    width: {
      default: "1rem",
      ":checked::after": ".35rem",
    },
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

function hasSemanticClass(className: string | undefined, name: string) {
  return className?.split(/\s+/).includes(name) ?? false;
}

function MdxLink({
  node: _,
  className,
  style,
  ...props
}: ComponentProps<"a"> & { node?: unknown }) {
  return (
    <a
      {...props}
      {...mergeStyleXProps(
        [
          styles.proseLink,
          hasSemanticClass(className, "anchor") && styles.headingAnchor,
        ],
        className,
        style,
      )}
    />
  );
}

function MdxUnorderedList({
  node: _,
  className,
  style,
  ...props
}: ComponentProps<"ul"> & { node?: unknown }) {
  return (
    <ul
      {...props}
      {...mergeStyleXProps(
        [hasSemanticClass(className, "contains-task-list") && styles.taskList],
        className,
        style,
      )}
    />
  );
}

function MdxListItem({
  node: _,
  className,
  style,
  ...props
}: ComponentProps<"li"> & { node?: unknown }) {
  return (
    <li
      {...props}
      {...mergeStyleXProps(
        [hasSemanticClass(className, "task-list-item") && styles.taskListItem],
        className,
        style,
      )}
    />
  );
}

function MdxInput({
  node: _,
  className,
  style,
  type,
  ...props
}: ComponentProps<"input"> & { node?: unknown }) {
  return (
    <input
      {...props}
      {...mergeStyleXProps(
        type === "checkbox" && styles.taskCheckbox,
        className,
        style,
      )}
      type={type}
    />
  );
}

export const mdxComponents: Record<string, ComponentType<any>> = {
  a: MdxLink,
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
  input: MdxInput,
  li: MdxListItem,
  ul: MdxUnorderedList,
};
