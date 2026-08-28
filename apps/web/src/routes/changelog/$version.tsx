import * as stylex from "@stylexjs/stylex";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";

import { ChangelogContent } from "@anlg/changelog";

import { SiteFooter } from "@/components/site-footer";
import { formatChangelogDate, getChangelogEntry } from "@/lib/changelog";
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
    maxWidth: "760px",
    paddingTop: {
      default: "2.5rem",
      "@media (width >= 48rem)": "3.5rem",
    },
    paddingBottom: {
      default: "3rem",
      "@media (width >= 48rem)": "4rem",
    },
  },
  style7: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style8: {
    "--tw-font-weight": "500",
    fontWeight: "500",
    "--tw-tracking": ".14em",
    letterSpacing: ".14em",
    textTransform: "uppercase",
  },
  style9: {
    marginTop: "1.25rem",
    fontSize: {
      default: "3rem",
      "@media (width >= 48rem)": "4.5rem",
    },
    lineHeight: {
      default: "1.02",
      "@media (width >= 48rem)": "1",
    },
    "--tw-leading": "1.02",
    "--tw-font-weight": "600",
    fontWeight: "600",
    textWrap: "balance",
    color: "#000",
  },
  style10: {
    marginTop: "1.5rem",
    maxWidth: "720px",
    fontSize: {
      default: "1.25rem",
      "@media (width >= 48rem)": "1.5rem",
    },
    lineHeight: {
      default: "2rem",
      "@media (width >= 48rem)": "2.25rem",
    },
    "--tw-leading": {
      default: "2rem",
      "@media (width >= 48rem)": "2.25rem",
    },
    color: "#4f4940",
  },
  style11: {
    maxWidth: "760px",
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#eee8df",
    paddingTop: {
      default: "2.5rem",
      "@media (width >= 48rem)": "3rem",
    },
  },
});
export const Route = createFileRoute("/changelog/$version")({
  component: Component,
  loader: async ({ params }) => {
    const entry = getChangelogEntry(params.version);
    if (!entry) {
      throw notFound();
    }
    return {
      entry,
    };
  },
  head: ({ loaderData }) => {
    const entry = loaderData?.entry;
    if (!entry) return {};
    const url = getCanonicalUrl(`/changelog/${entry.version}`);
    const description =
      entry.summary ?? `Release notes for Anarlog v${entry.version}.`;
    return {
      links: [
        {
          rel: "canonical",
          href: url,
        },
      ],
      meta: [
        // Per-version release notes are reference material for existing users,
        // not search targets. Indexing ~90 near-identical thin pages spends
        // crawl budget that belongs to the blog; /changelog/ stays the hub.
        {
          name: "robots",
          content: "noindex, follow",
        },
        {
          title: `Anarlog v${entry.version} Changelog`,
        },
        {
          name: "description",
          content: description,
        },
        {
          property: "og:title",
          content: `Anarlog v${entry.version} Changelog`,
        },
        {
          property: "og:description",
          content: description,
        },
        {
          property: "og:url",
          content: url,
        },
      ],
    };
  },
});
function Component() {
  const { entry } = Route.useLoaderData();
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

        <Link to="/changelog/" {...stylex.props(styles.style5)}>
          ← Changelog
        </Link>

        <header {...stylex.props(styles.style6)}>
          <div {...stylex.props(styles.style7)}>
            <span {...stylex.props(styles.style8)}>Release notes</span>
            {entry.date && (
              <>
                <span aria-hidden="true">·</span>
                <time dateTime={entry.date}>
                  {formatChangelogDate(entry.date)}
                </time>
              </>
            )}
          </div>
          <h1 {...stylex.props(styles.style9)}>Anarlog v{entry.version}</h1>
          {entry.summary && (
            <p {...stylex.props(styles.style10)}>{entry.summary}</p>
          )}
        </header>

        <article {...stylex.props(styles.style11)}>
          <ChangelogContent
            content={entry.content}
            className="changelog-prose"
          />
        </article>
      </div>

      <SiteFooter />
    </main>
  );
}
