import * as stylex from "@stylexjs/stylex";
import { isValidElement } from "react";
import { Streamdown } from "streamdown";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

function flattenTextContent(node: React.ReactNode): string {
  if (node == null || typeof node === "boolean") {
    return "";
  }

  if (
    typeof node === "string" ||
    typeof node === "number" ||
    typeof node === "bigint"
  ) {
    return String(node);
  }

  if (Array.isArray(node)) {
    return node.map(flattenTextContent).join("");
  }

  if (isValidElement<{ children?: React.ReactNode }>(node)) {
    return flattenTextContent(node.props.children);
  }

  return "";
}

const baseChangelogComponents = {
  h1: ({ children }: { children?: React.ReactNode }) => (
    <h1 {...stylex.props(styles.heading, styles.heading1)}>{children}</h1>
  ),
  h2: ({ children }: { children?: React.ReactNode }) => (
    <h2 {...stylex.props(styles.heading, styles.heading2)}>{children}</h2>
  ),
  h3: ({ children }: { children?: React.ReactNode }) => (
    <h3 {...stylex.props(styles.heading, styles.heading2)}>{children}</h3>
  ),
  h4: ({ children }: { children?: React.ReactNode }) => (
    <h4 {...stylex.props(styles.heading, styles.heading2)}>{children}</h4>
  ),
  h5: ({ children }: { children?: React.ReactNode }) => (
    <h5 {...stylex.props(styles.heading, styles.heading2)}>{children}</h5>
  ),
  h6: ({ children }: { children?: React.ReactNode }) => (
    <h6 {...stylex.props(styles.heading, styles.heading6)}>{children}</h6>
  ),
  p: ({ children }: { children?: React.ReactNode }) => (
    <p {...stylex.props(styles.body, styles.paragraph)}>{children}</p>
  ),
  strong: ({ children }: { children?: React.ReactNode }) => (
    <strong {...stylex.props(styles.strong)}>{children}</strong>
  ),
  em: ({ children }: { children?: React.ReactNode }) => (
    <em {...stylex.props(styles.emphasis)}>{children}</em>
  ),
  code: ({ children }: { children?: React.ReactNode }) => (
    <code {...stylex.props(styles.code)}>{children}</code>
  ),
  ul: ({ children }: { children?: React.ReactNode }) => (
    <ul {...stylex.props(styles.body, styles.unorderedList)}>{children}</ul>
  ),
  ol: ({ children }: { children?: React.ReactNode }) => (
    <ol {...stylex.props(styles.body, styles.orderedList)}>{children}</ol>
  ),
  li: ({ children }: { children?: React.ReactNode }) => (
    <li {...stylex.props(styles.listItem)}>{children}</li>
  ),
  blockquote: ({ children }: { children?: React.ReactNode }) => (
    <blockquote {...stylex.props(styles.blockquote)}>{children}</blockquote>
  ),
  a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
    <a
      {...stylex.props(styles.link)}
      href={href}
      target="_blank"
      rel="noopener noreferrer"
    >
      {children}
    </a>
  ),
  img: ({ src, alt }: { src?: string; alt?: string }) => (
    <img src={src} alt={alt} {...stylex.props(styles.image)} />
  ),
};

export const changelogComponents = {
  ...baseChangelogComponents,
  banner: ({
    title,
    variant,
    children,
  }: {
    title?: string;
    variant?: string;
    children?: React.ReactNode;
  }) => (
    <div
      data-changelog-banner
      data-variant={variant ?? "default"}
      {...stylex.props(
        styles.banner,
        variant === "warning"
          ? styles.warningBanner
          : variant === "info"
            ? styles.infoBanner
            : styles.defaultBanner,
      )}
    >
      {title && (
        <div
          data-changelog-banner-title
          {...stylex.props(
            styles.bannerTitle,
            variant === "warning"
              ? styles.warningBannerText
              : variant === "info"
                ? styles.infoBannerText
                : styles.defaultBannerText,
          )}
        >
          {title}
        </div>
      )}
      <div
        data-changelog-banner-content
        {...stylex.props(styles.body, styles.bannerContent)}
      >
        <Streamdown
          components={baseChangelogComponents}
          controls={false}
          isAnimating={false}
          linkSafety={{ enabled: false }}
        >
          {flattenTextContent(children)}
        </Streamdown>
      </div>
    </div>
  ),
};

const styles = stylex.create({
  heading: {
    color: "light-dark(#374151, #e7e5e4)",
    fontWeight: 600,
    marginBottom: "0.25rem",
    marginTop: {
      default: "1rem",
      ":first-child": 0,
    },
    minHeight: "1.5rem",
  },
  heading1: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
  },
  heading2: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  heading6: {
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  body: {
    color: colors.foreground,
  },
  paragraph: {
    lineHeight: "1.25rem",
    marginBottom: "0.25rem",
    textWrap: "wrap",
  },
  strong: {
    color: "light-dark(#374151, #e7e5e4)",
    fontWeight: 600,
  },
  emphasis: {
    fontStyle: "italic",
  },
  code: {
    borderColor: "light-dark(#e5e5e5, #57534e)",
    borderRadius: "0.4rem",
    borderStyle: "solid",
    borderWidth: "1px",
    color: "light-dark(#000, #e7e5e4)",
    fontFamily: "monospace",
    fontSize: "0.85rem",
    lineHeight: 1,
    paddingBlock: "0.15em",
    paddingInline: "0.3em",
    verticalAlign: "middle",
  },
  unorderedList: {
    listStyleType: "disc",
    marginBottom: "0.25rem",
    paddingLeft: "1.5rem",
  },
  orderedList: {
    listStyleType: "decimal",
    marginBottom: "0.25rem",
    paddingLeft: "1.5rem",
  },
  listItem: {
    marginBottom: "0.25rem",
  },
  blockquote: {
    borderColor: "light-dark(#000, #a8a29e)",
    borderLeftStyle: "solid",
    borderLeftWidth: "3px",
    marginBottom: "0.25rem",
    paddingLeft: "0.5rem",
  },
  link: {
    color: {
      default: "light-dark(#2563eb, #60a5fa)",
      ":hover": "light-dark(#1d4ed8, #93c5fd)",
    },
    textDecorationColor:
      "light-dark(rgb(96 165 250 / 0.4), rgb(59 130 246 / 0.5))",
    textDecorationLine: "underline",
    textUnderlineOffset: "2px",
  },
  image: {
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    marginBlock: "1.5rem",
  },
  banner: {
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    marginBottom: "0.5rem",
    paddingBottom: "1rem",
    paddingInline: "1.25rem",
    paddingTop: "1rem",
  },
  defaultBanner: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    color: colors.foreground,
  },
  warningBanner: {
    backgroundColor: "light-dark(#fffbeb, rgb(69 26 3 / 0.4))",
    borderColor: "light-dark(#fcd34d, #92400e)",
    color: "light-dark(#78350f, #fef3c7)",
  },
  infoBanner: {
    backgroundColor: "light-dark(#eff6ff, rgb(23 37 84 / 0.4))",
    borderColor: "light-dark(#93c5fd, #1e40af)",
    color: "light-dark(#1e3a8a, #dbeafe)",
  },
  bannerTitle: {
    fontSize: "0.875rem",
    fontWeight: 600,
    lineHeight: "1.25rem",
    marginBottom: "0.25rem",
  },
  defaultBannerText: {
    color: colors.foreground,
  },
  warningBannerText: {
    color: "light-dark(#78350f, #fef3c7)",
  },
  infoBannerText: {
    color: "light-dark(#1e3a8a, #dbeafe)",
  },
  bannerContent: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    marginBottom: {
      ":is(*) p:last-child": 0,
      ":is(*) ul:last-child": 0,
    },
  },
});
