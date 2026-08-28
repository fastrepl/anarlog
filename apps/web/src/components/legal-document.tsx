import { MDXContent } from "@content-collections/mdx/react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import type { Legal } from "content-collections";

import { colors, fonts, media } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

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
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style4: {
    marginBottom: "2.5rem",
  },
  style5: {
    fontFamily: fonts.hand,
    fontSize: "3rem",
    lineHeight: "1",
    "--tw-leading": "1",
    "--tw-font-weight": "600",
    fontWeight: "600",
    color: colors.foreground,
  },
  style6: {
    marginTop: "1.25rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    "--tw-leading": "2rem",
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
  },
});

const legalProseClassName =
  "prose prose-lg prose-stone prose-headings:font-hand prose-headings:font-semibold prose-headings:text-[#181613] prose-h2:text-4xl prose-h3:text-2xl prose-p:text-[#4f4940] prose-li:text-[#4f4940] prose-strong:text-[#181613] prose-a:text-[#181613] prose-a:underline hover:prose-a:text-[#4f4940] prose-blockquote:rounded-[3px] prose-blockquote:border prose-blockquote:border-[#eadfce] prose-blockquote:bg-[#fffaf0] prose-blockquote:px-6 prose-blockquote:py-1 prose-blockquote:font-normal prose-blockquote:not-italic prose-blockquote:text-[#363029] prose-blockquote:shadow-[0_18px_50px_rgba(68,54,36,0.08)] [&_blockquote_p]:before:content-none [&_blockquote_p]:after:content-none";
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

        <article {...mergeStyleXProps(styles.article, legalProseClassName)}>
          <MDXContent code={doc.mdx} components={mdxComponents} />
        </article>
      </div>
    </main>
  );
}
