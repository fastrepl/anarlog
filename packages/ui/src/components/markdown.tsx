import * as stylex from "@stylexjs/stylex";
import type { ComponentProps } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";

import { mergeStyleXProps } from "../lib/stylex";

type MarkdownElementProps<T extends keyof React.JSX.IntrinsicElements> =
  ComponentProps<T> & {
    node?: unknown;
  };

export const markdownComponents = {
  a: ({ node: _, className, style, ...props }: MarkdownElementProps<"a">) => (
    <a {...props} {...mergeStyleXProps(styles.link, className, style)} />
  ),
  blockquote: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"blockquote">) => (
    <blockquote
      {...props}
      {...mergeStyleXProps(styles.blockquote, className, style)}
    />
  ),
  code: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"code">) => (
    <code
      {...props}
      {...mergeStyleXProps(styles.codeBlock, className, style)}
    />
  ),
  del: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"del">) => (
    <del {...props} {...mergeStyleXProps(styles.deleted, className, style)} />
  ),
  em: ({ node: _, className, style, ...props }: MarkdownElementProps<"em">) => (
    <em {...props} {...mergeStyleXProps(styles.emphasis, className, style)} />
  ),
  h1: ({ node: _, className, style, ...props }: MarkdownElementProps<"h1">) => (
    <h1
      {...props}
      {...mergeStyleXProps([styles.heading, styles.heading1], className, style)}
    />
  ),
  h2: ({ node: _, className, style, ...props }: MarkdownElementProps<"h2">) => (
    <h2
      {...props}
      {...mergeStyleXProps([styles.heading, styles.heading2], className, style)}
    />
  ),
  h3: ({ node: _, className, style, ...props }: MarkdownElementProps<"h3">) => (
    <h3
      {...props}
      {...mergeStyleXProps([styles.heading, styles.heading3], className, style)}
    />
  ),
  h4: ({ node: _, className, style, ...props }: MarkdownElementProps<"h4">) => (
    <h4
      {...props}
      {...mergeStyleXProps([styles.heading, styles.heading4], className, style)}
    />
  ),
  h5: ({ node: _, className, style, ...props }: MarkdownElementProps<"h5">) => (
    <h5
      {...props}
      {...mergeStyleXProps([styles.heading, styles.heading4], className, style)}
    />
  ),
  h6: ({ node: _, className, style, ...props }: MarkdownElementProps<"h6">) => (
    <h6
      {...props}
      {...mergeStyleXProps([styles.heading, styles.heading4], className, style)}
    />
  ),
  hr: ({ node: _, className, style, ...props }: MarkdownElementProps<"hr">) => (
    <hr {...props} {...mergeStyleXProps(styles.rule, className, style)} />
  ),
  img: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"img">) => (
    <img {...props} {...mergeStyleXProps(styles.image, className, style)} />
  ),
  inlineCode: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"code">) => (
    <code
      {...props}
      {...mergeStyleXProps(styles.inlineCode, className, style)}
    />
  ),
  input: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"input">) => (
    <input
      {...props}
      {...mergeStyleXProps(styles.checkbox, className, style)}
    />
  ),
  li: ({ node: _, className, style, ...props }: MarkdownElementProps<"li">) => (
    <li {...props} {...mergeStyleXProps(styles.listItem, className, style)} />
  ),
  ol: ({ node: _, className, style, ...props }: MarkdownElementProps<"ol">) => (
    <ol
      {...props}
      {...mergeStyleXProps([styles.list, styles.orderedList], className, style)}
    />
  ),
  p: ({ node: _, className, style, ...props }: MarkdownElementProps<"p">) => (
    <p {...props} {...mergeStyleXProps(styles.paragraph, className, style)} />
  ),
  pre: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"pre">) => (
    <pre {...props} {...mergeStyleXProps(styles.pre, className, style)} />
  ),
  strong: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"strong">) => (
    <strong {...props} {...mergeStyleXProps(styles.strong, className, style)} />
  ),
  table: ({
    node: _,
    className,
    style,
    ...props
  }: MarkdownElementProps<"table">) => (
    <div {...stylex.props(styles.tableViewport)}>
      <table {...props} {...mergeStyleXProps(styles.table, className, style)} />
    </div>
  ),
  td: ({ node: _, className, style, ...props }: MarkdownElementProps<"td">) => (
    <td {...props} {...mergeStyleXProps(styles.tableCell, className, style)} />
  ),
  th: ({ node: _, className, style, ...props }: MarkdownElementProps<"th">) => (
    <th
      {...props}
      {...mergeStyleXProps(
        [styles.tableCell, styles.tableHeading],
        className,
        style,
      )}
    />
  ),
  ul: ({ node: _, className, style, ...props }: MarkdownElementProps<"ul">) => (
    <ul
      {...props}
      {...mergeStyleXProps(
        [styles.list, styles.unorderedList],
        className,
        style,
      )}
    />
  ),
};

const styles = stylex.create({
  blockquote: {
    borderLeftColor: colors.border,
    borderLeftStyle: "solid",
    borderLeftWidth: "3px",
    color: colors.mutedForeground,
    marginBlock: "0.75rem",
    paddingLeft: "1rem",
  },
  checkbox: {
    accentColor: colors.primary,
    marginRight: "0.5rem",
  },
  codeBlock: {
    fontFamily: fonts.mono,
    fontSize: "0.875em",
  },
  deleted: {
    textDecorationLine: "line-through",
  },
  emphasis: {
    fontStyle: "italic",
  },
  heading: {
    color: colors.foreground,
    fontWeight: 600,
    lineHeight: 1.25,
    marginBottom: "0.5rem",
    marginTop: "1rem",
  },
  heading1: {
    fontSize: "1.5rem",
  },
  heading2: {
    fontSize: "1.25rem",
  },
  heading3: {
    fontSize: "1.125rem",
  },
  heading4: {
    fontSize: "1rem",
  },
  image: {
    borderRadius: radii.lg,
    height: "auto",
    marginBlock: "0.75rem",
    maxWidth: "100%",
  },
  inlineCode: {
    backgroundColor: colors.muted,
    borderRadius: radii.sm,
    fontFamily: fonts.mono,
    fontSize: "0.875em",
    paddingBlock: "0.125rem",
    paddingInline: "0.25rem",
  },
  link: {
    color: {
      default: colors.primary,
      ":hover": colors.mutedForeground,
    },
    textDecorationLine: "underline",
    textUnderlineOffset: "2px",
  },
  list: {
    marginBlock: "0.5rem",
    paddingLeft: "1.5rem",
  },
  listItem: {
    marginBlock: "0.25rem",
  },
  orderedList: {
    listStyleType: "decimal",
  },
  paragraph: {
    lineHeight: 1.6,
    marginBlock: "0.5rem",
  },
  pre: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    fontFamily: fonts.mono,
    marginBlock: "0.75rem",
    overflowX: "auto",
    padding: "1rem",
    whiteSpace: "pre-wrap",
  },
  rule: {
    borderColor: colors.border,
    borderStyle: "solid",
    borderTopWidth: "1px",
    borderRightWidth: "0",
    borderBottomWidth: "0",
    borderLeftWidth: "0",
    marginBlock: "1rem",
  },
  strong: {
    color: colors.foreground,
    fontWeight: 600,
  },
  table: {
    borderCollapse: "collapse",
    minWidth: "100%",
  },
  tableCell: {
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    verticalAlign: "top",
  },
  tableHeading: {
    backgroundColor: colors.muted,
    color: colors.foreground,
    fontWeight: 600,
  },
  tableViewport: {
    marginBlock: "0.75rem",
    overflowX: "auto",
    width: "100%",
  },
  unorderedList: {
    listStyleType: "disc",
  },
});
