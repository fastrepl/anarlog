import { MDXContent } from "@content-collections/mdx/react";
import { ArrowRight } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import type { Article, ArticleSummary } from "content-collections";
import {
  Children,
  cloneElement,
  isValidElement,
  type ComponentProps,
  type ReactElement,
  type ReactNode,
} from "react";

import { fonts, radii } from "@anlg/design-system/tokens.stylex";

import { mdxComponents } from "@/components/mdx-components";
import { SiteFooter } from "@/components/site-footer";
import { formatBlogDate } from "@/lib/blog-date";
import {
  getBlogOgImageUrl,
  getBlogPostingJsonLd,
  getBreadcrumbListJsonLd,
  getCanonicalUrl,
  getStructuredDataGraph,
} from "@/lib/seo";
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
    marginTop: "4rem",
    display: "inline-block",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
  },
  style6: {
    paddingTop: "2.5rem",
    paddingBottom: "3rem",
  },
  style7: {
    fontFamily: fonts.hand,
    fontSize: {
      default: "3rem",
      "@media (width >= 48rem)": "3.75rem",
    },
    lineHeight: {
      default: 1.02,
      "@media (width >= 48rem)": 1,
    },
    fontWeight: 600,
    letterSpacing: 0,
    textWrap: "balance",
    color: "#000",
  },
  style8: {
    marginTop: "1.5rem",
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style9: {
    marginBottom: "3rem",
    borderBlockStyle: "solid",
    borderBlockWidth: "1px",
    borderColor: "#eee8df",
    paddingBlock: "1.25rem",
  },
  style10: {
    fontFamily: fonts.hand,
    fontSize: "1.125rem",
    lineHeight: "1.75rem",
    fontWeight: 600,
    letterSpacing: 0,
    color: "#756b5d",
  },
  style11: {
    marginTop: ".75rem",
    fontFamily: fonts.hand,
    fontSize: {
      default: "1.25rem",
      "@media (width >= 48rem)": "1.5rem",
    },
    lineHeight: {
      default: "1.75rem",
      "@media (width >= 48rem)": "2rem",
    },
    fontWeight: 600,
    color: "#363029",
  },
  style12: {
    backgroundColor: {
      ":is(*) pre": "#2b2523",
      ":is(*) pre code": "transparent",
      ":is(*) th": "#eee7db",
      ":is(*) td": "#faf7f1",
      ":is(*) tbody tr td:first-child": "#f3eee6",
    },
    borderRadius: {
      ":is(*) img": ".375rem",
      ":is(*) pre": ".375rem",
      ":is(*) pre code": 0,
    },
    borderCollapse: {
      ":is(*) table": "separate",
    },
    borderSpacing: {
      ":is(*) table": "0 .25rem",
    },
    color: {
      default: "#4f4940",
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": "#756b5d",
      ":is(*) :is(p, li)": "#363029",
      ":is(*) strong": "#181613",
      ":is(*) a": "#181613",
      ":is(*) a:hover": "#4f4940",
      ":is(*) pre code": "#f8f4ef",
      ":is(*) th": "#4f4940",
      ":is(*) td": "#363029",
    },
    display: {
      ":is(*) pre code": "block",
    },
    fontFamily: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": fonts.hand,
      ":is(*) :is(table, th, td)": fonts.sans,
      ":is(*) pre code": fonts.mono,
    },
    fontSize: {
      default: "1.125rem",
      ":is(*) :is(p, li)": "1.125rem",
      ":is(*) pre code": ".9375rem",
      ":is(*) table": "1rem",
      ":is(*) th": "1.0625rem",
      ":is(*) h2": "clamp(2.125rem, 1.75rem + 1.25vw, 2.5rem)",
      ":is(*) h3": "clamp(1.75rem, 1.5rem + .85vw, 2rem)",
      ":is(*) h4": "1.375rem",
    },
    fontWeight: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": 600,
      ":is(*) th": 650,
      ":is(*) tbody tr td:first-child": 600,
    },
    letterSpacing: {
      ":is(*) :is(h2, h3, h4)": 0,
    },
    lineHeight: {
      default: 1.6,
      ":is(*) :is(p, li)": 1.6,
      ":is(*) pre code": 1.65,
      ":is(*) th": 1.25,
      ":is(*) td": 1.35,
      ":is(*) :is(h2, h3)": 1.05,
      ":is(*) h4": 1.1,
    },
    listStyleType: {
      ":is(*) ul": "disc",
      ":is(*) ol": "decimal",
    },
    marginBlock: {
      ":is(*) :is(p, ul, ol, blockquote, figure, hr)": "1.25rem",
      ":is(*) img": "2rem",
      ":is(*) hr": "3rem",
      ":is(*) pre": "1.5rem",
    },
    maxWidth: {
      default: "none",
      ":is(*) img": "100%",
    },
    marginBottom: {
      ":is(*) h2": "1.25rem",
      ":is(*) h3": "1rem",
      ":is(*) h4": ".75rem",
    },
    marginTop: {
      ":is(*) h2": "4rem",
      ":is(*) h3": "3.5rem",
      ":is(*) h4": "2.5rem",
      ":is(*) :is(h2 + h3, h3 + h4)": "1.5rem",
    },
    minWidth: {
      ":is(*) table": "100%",
    },
    overflowWrap: {
      ":is(*) pre code": "anywhere",
    },
    overflowX: {
      ":is(*) pre": "hidden",
    },
    padding: {
      ":is(*) pre code": 0,
    },
    paddingBlock: {
      ":is(*) :is(th, td)": ".625rem",
      ":is(*) pre": "1rem",
    },
    paddingInline: {
      ":is(*) :is(th, td)": ".75rem",
      ":is(*) pre": "1.125rem",
    },
    paddingLeft: {
      ":is(*) :is(ul, ol)": "1.5rem",
      ":is(*) blockquote": "1.25rem",
    },
    textAlign: {
      ":is(*) th": "left",
    },
    textDecorationLine: {
      ":is(*) a": "underline",
      ":is(*) :is(h2, h3, h4) a": "none",
    },
    verticalAlign: {
      ":is(*) td": "top",
    },
    whiteSpace: {
      ":is(*) pre code": "pre-wrap",
      ":is(*) table": "nowrap",
    },
    width: {
      ":is(*) table": "max-content",
    },
    wordBreak: {
      ":is(*) pre code": "normal",
    },
  },
  style13: {
    marginTop: "5rem",
  },
  style14: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: "2.25rem",
    fontWeight: 600,
    letterSpacing: 0,
    color: "#756b5d",
  },
  style15: {
    marginTop: "1.25rem",
    display: "grid",
    gap: "1.25rem",
    gridTemplateColumns: {
      default: null,
      "@media (width >= 48rem)": "repeat(3, minmax(0, 1fr))",
    },
  },
  style16: {
    display: "block",
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#eee8df",
    paddingTop: "1rem",
  },
  style17: {
    fontFamily: fonts.hand,
    fontSize: "1.25rem",
    lineHeight: "1.5rem",
    fontWeight: 600,
    color: {
      default: "#756b5d",
      [stylex.when.ancestor(":hover")]: "#4f4940",
    },
  },
  style18: {
    marginTop: ".5rem",
    display: "block",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#756b5d",
  },
  style19: {
    marginBlock: "1.5rem",
    overflowX: "auto",
  },
  style20: {
    marginTop: "5rem",
    borderRadius: ".125rem",
    borderStyle: "solid",
    borderWidth: "1px",
    backgroundColor: "#faf7f1",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "1.75rem",
    },
    paddingBlock: "2rem",
  },
  style21: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 48rem)": "row",
    },
    gap: "1.25rem",
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 48rem)": "space-between",
    },
  },
  style22: {
    fontFamily: fonts.hand,
    fontSize: {
      default: "1.875rem",
      "@media (width >= 48rem)": "2.25rem",
    },
    lineHeight: {
      default: 1,
      "@media (width >= 48rem)": "2.5rem",
    },
    fontWeight: 600,
    letterSpacing: 0,
    color: "#756b5d",
  },
  style23: {
    marginTop: ".75rem",
    fontSize: "1rem",
    lineHeight: "1.75rem",
    color: "#4f4940",
  },
  style24: {
    display: "inline-flex",
    height: "3rem",
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    gap: ".5rem",
    borderRadius: radii.full,
    backgroundColor: {
      default: "#181613",
      ":hover": "#363029",
    },
    paddingInline: "1.25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 600,
    color: "#fff",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
});
const blogMdxComponents = {
  ...mdxComponents,
  table: BlogTable,
};
export const Route = createFileRoute("/blog/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const { allArticles, allArticleSummaries } =
      await import("content-collections");
    const article = allArticles.find((a: Article) => a.slug === params.slug);
    if (!article) {
      throw notFound();
    }
    return {
      article,
      relatedArticles: getRelatedArticles(article, allArticleSummaries),
    };
  },
  head: ({ loaderData }) => {
    const article = loaderData?.article;
    if (!article) return {};
    const url = getCanonicalUrl(`/blog/${article.slug}`);
    const imageUrl = getBlogOgImageUrl(article.slug);
    const authors = Array.isArray(article.author)
      ? article.author
      : [article.author];
    return {
      links: [
        {
          rel: "canonical",
          href: url,
        },
      ],
      meta: [
        {
          title: article.meta_title || article.title,
        },
        {
          name: "description",
          content: article.meta_description,
        },
        {
          property: "og:title",
          content: article.meta_title || article.title,
        },
        {
          property: "og:description",
          content: article.meta_description,
        },
        {
          property: "og:url",
          content: url,
        },
        {
          property: "og:type",
          content: "article",
        },
        {
          property: "og:image",
          content: imageUrl,
        },
        {
          property: "og:image:type",
          content: "image/png",
        },
        {
          property: "og:image:width",
          content: "1200",
        },
        {
          property: "og:image:height",
          content: "630",
        },
        {
          property: "og:image:alt",
          content: `Preview of ${article.title}`,
        },
        {
          name: "twitter:card",
          content: "summary_large_image",
        },
        {
          name: "twitter:title",
          content: article.meta_title || article.title,
        },
        {
          name: "twitter:description",
          content: article.meta_description,
        },
        {
          name: "twitter:image",
          content: imageUrl,
        },
        {
          name: "twitter:image:alt",
          content: `Preview of ${article.title}`,
        },
      ],
      scripts: [
        {
          type: "application/ld+json",
          children: JSON.stringify(
            getStructuredDataGraph([
              getBlogPostingJsonLd({
                url,
                headline: article.title,
                description: article.meta_description,
                image: imageUrl,
                datePublished: article.date,
                authors,
              }),
              getBreadcrumbListJsonLd([
                {
                  name: "Home",
                  item: getCanonicalUrl(),
                },
                {
                  name: "Blog",
                  item: getCanonicalUrl("/blog"),
                },
                {
                  name: article.title,
                  item: url,
                },
              ]),
            ]),
          ),
        },
      ],
    };
  },
});
function Component() {
  const { article, relatedArticles } = Route.useLoaderData();
  const authors = Array.isArray(article.author)
    ? article.author.join(", ")
    : article.author;
  const tldr = article.meta_description.trim();
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

        <Link to="/blog/" {...stylex.props(styles.style5)}>
          ← Blog
        </Link>

        <header {...stylex.props(styles.style6)}>
          <h1 {...stylex.props(styles.style7)}>{article.title}</h1>
          <div {...stylex.props(styles.style8)}>
            <span>{authors}</span>
            <span>·</span>
            <time dateTime={article.date}>
              {formatBlogDate(article.date, "long")}
            </time>
          </div>
        </header>

        {tldr && (
          <aside aria-label="TLDR" {...stylex.props(styles.style9)}>
            <p {...stylex.props(styles.style10)}>TL;DR</p>
            <p {...stylex.props(styles.style11)}>{tldr}</p>
          </aside>
        )}

        <article {...stylex.props(styles.style12)}>
          <MDXContent code={article.mdx} components={blogMdxComponents} />
        </article>

        <RelatedArticles articles={relatedArticles} />
        <BlogArticleCta />
      </div>

      <SiteFooter />
    </main>
  );
}
function getRelatedArticles(
  article: Article,
  articleSummaries: ArticleSummary[],
) {
  const sortedArticles = [...articleSummaries].sort(
    (a, b) =>
      new Date(b.date).getTime() - new Date(a.date).getTime() ||
      a.slug.localeCompare(b.slug),
  );
  const articleIndex = sortedArticles.findIndex(
    (candidate) => candidate.slug === article.slug,
  );
  if (articleIndex === -1) {
    return [];
  }
  const neighbors = [
    sortedArticles[articleIndex - 1],
    sortedArticles[articleIndex + 1],
  ].filter((candidate): candidate is ArticleSummary => Boolean(candidate));
  const sameCategory = sortedArticles.find(
    (candidate) =>
      candidate.slug !== article.slug &&
      candidate.category === article.category &&
      !neighbors.some((neighbor) => neighbor.slug === candidate.slug),
  );
  const fallback = sortedArticles.find(
    (candidate) =>
      candidate.slug !== article.slug &&
      !neighbors.some((neighbor) => neighbor.slug === candidate.slug),
  );
  return [...neighbors, sameCategory ?? fallback].filter(
    (candidate): candidate is ArticleSummary => Boolean(candidate),
  );
}
function RelatedArticles({ articles }: { articles: ArticleSummary[] }) {
  if (articles.length === 0) {
    return null;
  }
  return (
    <aside
      aria-labelledby="keep-reading-heading"
      {...stylex.props(styles.style13)}
    >
      <h2 id="keep-reading-heading" {...stylex.props(styles.style14)}>
        Keep reading
      </h2>
      <ul {...stylex.props(styles.style15)}>
        {articles.map((relatedArticle) => (
          <li key={relatedArticle.slug}>
            <Link
              to="/blog/$slug/"
              params={{
                slug: relatedArticle.slug,
              }}
              {...stylex.props(styles.style16, stylex.defaultMarker())}
            >
              <p {...stylex.props(styles.style17)}>{relatedArticle.title}</p>
              <time
                dateTime={relatedArticle.date}
                {...stylex.props(styles.style18)}
              >
                {formatBlogDate(relatedArticle.date, "short")}
              </time>
            </Link>
          </li>
        ))}
      </ul>
    </aside>
  );
}
function BlogTable({ children, ...props }: ComponentProps<"table">) {
  return (
    <div {...stylex.props(styles.style19)}>
      <table {...props}>{normalizeTableChildren(children)}</table>
    </div>
  );
}
type ElementWithChildren = ReactElement<{
  children?: ReactNode;
}>;
function normalizeTableChildren(children: ReactNode) {
  return Children.toArray(children).map((child) => {
    const element = getElementWithChildren(child);
    if (!element) {
      return child;
    }
    if (element.type === "thead") {
      const rows = Children.toArray(element.props.children);
      return rows.length > 0 && rows.every(isBlankTableRow) ? null : child;
    }
    if (element.type !== "tbody") {
      return child;
    }
    const rows = Children.toArray(element.props.children);
    const visibleRows = rows.filter((row) => !isBlankTableRow(row));
    if (visibleRows.length === rows.length) {
      return child;
    }
    return visibleRows.length > 0
      ? cloneElement(element, undefined, visibleRows)
      : null;
  });
}
function isBlankTableRow(row: ReactNode) {
  const element = getElementWithChildren(row);
  if (!element || element.type !== "tr") {
    return false;
  }
  const cells = Children.toArray(element.props.children);
  return cells.length > 0 && cells.every(isBlankTableCell);
}
function isBlankTableCell(cell: ReactNode) {
  const element = getElementWithChildren(cell);
  if (!element || (element.type !== "td" && element.type !== "th")) {
    return false;
  }
  return isBlankNode(element.props.children);
}
function isBlankNode(node: ReactNode): boolean {
  if (node === null || node === undefined || typeof node === "boolean") {
    return true;
  }
  if (typeof node === "string" || typeof node === "number") {
    return (
      String(node)
        .replace(/\u00a0/g, " ")
        .trim() === ""
    );
  }
  const element = getElementWithChildren(node);
  if (element) {
    const { children } = element.props;
    if (
      children === null ||
      children === undefined ||
      typeof children === "boolean"
    ) {
      return false;
    }
    return isBlankNode(children);
  }
  const children = Children.toArray(node);
  return children.length === 0 || children.every(isBlankNode);
}
function getElementWithChildren(node: ReactNode): ElementWithChildren | null {
  return isValidElement<{
    children?: ReactNode;
  }>(node)
    ? node
    : null;
}
function BlogArticleCta() {
  return (
    <aside aria-label="Try Anarlog for free" {...stylex.props(styles.style20)}>
      <div {...stylex.props(styles.style21)}>
        <div>
          <p {...stylex.props(styles.style22)}>
            Take notes without inviting a bot
          </p>
          <p {...stylex.props(styles.style23)}>
            Try Anarlog for private, local-first meeting notes on your desktop.
          </p>
        </div>
        <Link to="/download/" {...stylex.props(styles.style24)}>
          Try for free
          <ArrowRight size={17} weight="bold" aria-hidden="true" />
        </Link>
      </div>
    </aside>
  );
}
