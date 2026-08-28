import { MDXContent } from "@content-collections/mdx/react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import type { Legal } from "content-collections";

import { colors, fonts, media } from "@anlg/design-system/tokens.stylex";

import { getCanonicalUrl } from "@/lib/seo";

import { mdxComponents } from "./mdx-components";
const styles = stylex.create({
  style1: {
    minHeight: "100vh",
    backgroundColor: colors.card,
    color: colors.foreground,
  },
  style2: {
    marginInline: "auto",
    width: "100%",
    maxWidth: "700px",
    paddingInline: {
      default: "1.25rem",
      [media.md]: "2rem",
    },
    paddingBlock: {
      default: "3.5rem",
      [media.md]: "4rem",
    },
  },
  style3: {
    marginBottom: "2.5rem",
    display: "inline-block",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style4: {
    marginBottom: "2.5rem",
  },
  style5: {
    fontFamily: fonts.hand,
    fontSize: "3rem",
    lineHeight: 1,
    fontWeight: 600,
    color: colors.foreground,
  },
  style6: {
    marginTop: "1.25rem",
    maxWidth: "42rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    color: colors.mutedForeground,
  },
  style7: {
    marginTop: ".75rem",
    display: "block",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  article: {
    maxWidth: "none",
    backgroundColor: {
      ":is(*) blockquote": "#fffaf0",
    },
    borderColor: {
      ":is(*) blockquote": "#eadfce",
    },
    borderRadius: {
      ":is(*) blockquote": "3px",
    },
    borderStyle: {
      ":is(*) blockquote": "solid",
    },
    borderWidth: {
      ":is(*) blockquote": "1px",
    },
    boxShadow: {
      ":is(*) blockquote": "0 18px 50px rgb(68 54 36 / 0.08)",
    },
    color: {
      default: "#4f4940",
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": "#181613",
      ":is(*) :is(p, li)": "#4f4940",
      ":is(*) strong": "#181613",
      ":is(*) a": "#181613",
      ":is(*) a:hover": "#4f4940",
      ":is(*) blockquote": "#363029",
    },
    content: {
      ":is(*) blockquote p::before": "none",
      ":is(*) blockquote p::after": "none",
    },
    fontFamily: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": fonts.hand,
    },
    fontSize: {
      default: "1.125rem",
      ":is(*) h2": "2.25rem",
      ":is(*) h3": "1.5rem",
      ":is(*) h4": "1.125rem",
    },
    fontStyle: {
      ":is(*) blockquote": "normal",
    },
    fontWeight: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": 600,
      ":is(*) strong": 600,
      ":is(*) blockquote": 400,
    },
    lineHeight: {
      default: 1.75,
      ":is(*) :is(p, li)": 1.75,
      ":is(*) :is(h2, h3, h4)": 1.25,
    },
    listStyleType: {
      ":is(*) ul": "disc",
      ":is(*) ol": "decimal",
    },
    marginBlock: {
      ":is(*) :is(p, ul, ol, pre, table)": "1.25rem",
      ":is(*) blockquote": "2rem",
      ":is(*) hr": "3rem",
    },
    marginBottom: {
      ":is(*) :is(h2, h3, h4)": ".75rem",
    },
    marginTop: {
      ":is(*) h2": "2.5rem",
      ":is(*) h3": "2rem",
      ":is(*) h4": "1.5rem",
      ":is(*) > :first-child": 0,
    },
    paddingBlock: {
      ":is(*) blockquote": ".25rem",
    },
    paddingInline: {
      ":is(*) blockquote": "1.5rem",
    },
    paddingLeft: {
      ":is(*) :is(ul, ol)": "1.5rem",
    },
    textDecorationLine: {
      ":is(*) a": "underline",
    },
  },
});

export function legalHead(doc: Legal, path: "/privacy" | "/terms") {
  const url = getCanonicalUrl(path);
  return {
    links: [
      {
        rel: "canonical",
        href: url,
      },
    ],
    meta: [
      {
        title: `${doc.title} — Anarlog`,
      },
      {
        name: "description",
        content: doc.summary || doc.title,
      },
      {
        property: "og:title",
        content: doc.title,
      },
      {
        property: "og:description",
        content: doc.summary || doc.title,
      },
      {
        property: "og:url",
        content: url,
      },
    ],
  };
}
export function LegalDocument({ doc }: { doc: Legal }) {
  return (
    <main {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <Link to="/" {...stylex.props(styles.style3)}>
          ← Home
        </Link>

        <header {...stylex.props(styles.style4)}>
          <h1 {...stylex.props(styles.style5)}>{doc.title}</h1>
          {doc.summary ? (
            <p {...stylex.props(styles.style6)}>{doc.summary}</p>
          ) : null}
          <time dateTime={doc.date} {...stylex.props(styles.style7)}>
            Last updated{" "}
            {new Date(doc.date).toLocaleDateString("en-US", {
              month: "long",
              day: "numeric",
              year: "numeric",
            })}
          </time>
        </header>

        <article {...stylex.props(styles.article)}>
          <MDXContent code={doc.mdx} components={mdxComponents} />
        </article>
      </div>
    </main>
  );
}
