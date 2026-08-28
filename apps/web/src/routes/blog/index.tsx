import * as stylex from "@stylexjs/stylex";
import { createFileRoute, Link } from "@tanstack/react-router";
import allArticleSummaries from "article-summaries";

import { fonts } from "@anlg/design-system/tokens.stylex";

import { SiteFooter } from "@/components/site-footer";
import { formatBlogDate } from "@/lib/blog-date";
import { getCanonicalUrl } from "@/lib/seo";
const styles = stylex.create({
  style1: {
    minHeight: "100vh",
    backgroundColor: "#fff",
    color: "#181613",
  },
  style2: {
    marginInline: "auto",
    width: "100%",
    maxWidth: "860px",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    paddingBlock: {
      default: "2rem",
      "@media (width >= 48rem)": "3rem",
    },
  },
  style3: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "1.5rem",
  },
  style4: {
    height: "2.25rem",
    width: "auto",
  },
  style5: {
    paddingTop: {
      default: "6rem",
      "@media (width >= 48rem)": "8rem",
    },
    paddingBottom: "4rem",
  },
  style6: {
    fontFamily: fonts.hand,
    fontSize: {
      default: "3.75rem",
      "@media (width >= 48rem)": "6rem",
    },
    lineHeight: {
      default: 0.98,
      "@media (width >= 48rem)": 1,
    },
    fontWeight: 600,
    letterSpacing: 0,
    textWrap: "balance",
    color: "#000",
  },
  style7: {
    marginTop: "1.5rem",
    fontSize: "1.25rem",
    lineHeight: "2.25rem",
    color: "#363029",
  },
  style8: {
    display: "grid",
    gap: "2.25rem",
  },
  style9: {
    display: "block",
  },
  style10: {
    display: "grid",
    gap: ".75rem",
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#eee8df",
    paddingTop: "1.5rem",
  },
  style11: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: 1.05,
    fontWeight: 600,
    letterSpacing: 0,
    textWrap: "balance",
    color: {
      default: "#756b5d",
      [stylex.when.ancestor(":hover")]: "#4f4940",
    },
  },
  style12: {
    WebkitLineClamp: 2,
    WebkitBoxOrient: "vertical",
    display: "-webkit-box",
    overflow: "hidden",
    lineHeight: "1.75rem",
    color: "#4f4940",
  },
  style13: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#756b5d",
  },
});
export const Route = createFileRoute("/blog/")({
  component: Component,
  head: () => ({
    links: [
      {
        rel: "canonical",
        href: getCanonicalUrl("/blog"),
      },
    ],
    meta: [
      {
        title: "Anarlog Blog",
      },
      {
        name: "description",
        content:
          "Guides for AI meeting notes, privacy research, and engineering notes from the Anarlog team.",
      },
      {
        property: "og:title",
        content: "Anarlog Blog",
      },
      {
        property: "og:url",
        content: getCanonicalUrl("/blog"),
      },
    ],
  }),
});
function Component() {
  const sortedArticles = [...allArticleSummaries].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
  );
  return (
    <main {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <header {...stylex.props(styles.style3)}>
          <Link to="/" aria-label="Anarlog home">
            <img
              src="/logo.svg"
              alt="Anarlog"
              {...stylex.props(styles.style4)}
            />
          </Link>
        </header>

        <section {...stylex.props(styles.style5)}>
          <h1 {...stylex.props(styles.style6)}>Blog</h1>
          <p {...stylex.props(styles.style7)}>
            Notes on private meetings, local-first workflows, open source, and
            AI you can run on your own terms.
          </p>
        </section>

        <ul {...stylex.props(styles.style8)}>
          {sortedArticles.map((article) => (
            <li key={article.slug}>
              <Link
                to="/blog/$slug/"
                params={{
                  slug: article.slug,
                }}
                {...stylex.props(styles.style9, stylex.defaultMarker())}
              >
                <article {...stylex.props(styles.style10)}>
                  <h2 {...stylex.props(styles.style11)}>{article.title}</h2>
                  {article.meta_description && (
                    <p {...stylex.props(styles.style12)}>
                      {article.meta_description}
                    </p>
                  )}
                  <div {...stylex.props(styles.style13)}>
                    <span>
                      {Array.isArray(article.author)
                        ? article.author.join(", ")
                        : article.author}
                    </span>
                    <span>·</span>
                    <time dateTime={article.date}>
                      {formatBlogDate(article.date, "short")}
                    </time>
                  </div>
                </article>
              </Link>
            </li>
          ))}
        </ul>
      </div>

      <SiteFooter />
    </main>
  );
}
