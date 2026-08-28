import * as stylex from "@stylexjs/stylex";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";

import { ChangelogContent } from "@anlg/changelog";
import { fonts } from "@anlg/design-system/tokens.stylex";

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
    fontWeight: 500,
    letterSpacing: ".14em",
    textTransform: "uppercase",
  },
  style9: {
    marginTop: "1.25rem",
    fontFamily: fonts.hand,
    fontSize: {
      default: "3rem",
      "@media (width >= 48rem)": "4.5rem",
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
  content: {
    backgroundColor: {
      ":is(*) code": "#f7f4ef",
      ":is(*) [data-changelog-banner]": "#f3f7fc",
      ':is(*) [data-changelog-banner][data-variant="warning"]': "#fcf8ed",
    },
    borderColor: {
      ":is(*) code": "#ded7cc",
      ":is(*) img": "#eee8df",
      ":is(*) [data-changelog-banner]": "#c9d9ec",
      ':is(*) [data-changelog-banner][data-variant="warning"]': "#e8d6a8",
    },
    borderLeftColor: {
      ":is(*) blockquote": "#b1a798",
    },
    borderRadius: {
      ":is(*) img": ".75rem",
      ":is(*) [data-changelog-banner]": ".75rem",
    },
    color: {
      default: "#4f4940",
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": "#756b5d",
      ":is(*) :is(p, li, ul, ol)": "#4f4940",
      ":is(*) li::marker": "#b1a798",
      ":is(*) strong": "#181613",
      ":is(*) code": "#363029",
      ":is(*) a": "#181613",
      ":is(*) a:hover": "#756b5d",
      ":is(*) blockquote": "#756b5d",
      ":is(*) [data-changelog-banner]": "#29466c",
      ':is(*) [data-changelog-banner][data-variant="warning"]': "#684f1e",
      ":is(*) [data-changelog-banner-title]": "inherit",
      ":is(*) [data-changelog-banner-content] :is(p, ul, ol, li, strong, a, a:hover, li::marker)":
        "inherit",
    },
    fontFamily: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": fonts.hand,
    },
    fontSize: {
      default: "1.0625rem",
      ":is(*) :is(h1, h2)": "clamp(2rem, 1.75rem + .85vw, 2.25rem)",
      ":is(*) :is(h3, h4, h5, h6)": "1.625rem",
      ":is(*) :is(p, li)": "1.0625rem",
      ":is(*) code": ".875em",
      ":is(*) [data-changelog-banner-title]": ".75rem",
      ":is(*) [data-changelog-banner-content] :is(p, ul, ol, li, strong, a)":
        "1rem",
    },
    fontWeight: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": 600,
      ":is(*) strong": 650,
      ":is(*) [data-changelog-banner-title]": 700,
    },
    letterSpacing: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": 0,
      ":is(*) [data-changelog-banner-title]": ".12em",
    },
    lineHeight: {
      default: 1.75,
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": 1.1,
      ":is(*) :is(p, li)": 1.75,
      ":is(*) [data-changelog-banner-content] :is(p, ul, ol, li, strong, a)": 1.6,
    },
    marginBlock: {
      ":is(*) blockquote": "2rem",
      ":is(*) img": "2.5rem",
    },
    marginBottom: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": "1rem",
      ":is(*) p": "1.25rem",
      ":is(*) :is(ul, ol)": "2rem",
      ":is(*) li": ".75rem",
      ":is(*) [data-changelog-banner]": "3rem",
      ":is(*) [data-changelog-banner-title]": ".375rem",
      ":is(*) [data-changelog-banner-content] :is(ul, ol, li)": ".25rem",
      ":is(*) [data-changelog-banner-content] :is(p, ul, ol):last-child": 0,
    },
    marginTop: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": "3.5rem",
      ":is(*) > :first-child:is(h1, h2, h3)": 0,
    },
    minHeight: {
      ":is(*) :is(h1, h2, h3, h4, h5, h6)": 0,
    },
    paddingBlock: {
      ":is(*) [data-changelog-banner]": "1.25rem",
    },
    paddingInline: {
      ":is(*) [data-changelog-banner]": "1.5rem",
    },
    paddingLeft: {
      ":is(*) :is(ul, ol)": "1.5rem",
      ":is(*) li": ".25rem",
      ":is(*) blockquote": "1.25rem",
    },
    textDecorationColor: {
      ":is(*) a": "#b1a798",
      ":is(*) [data-changelog-banner-content] a": "currentColor",
    },
    textTransform: {
      ":is(*) [data-changelog-banner-title]": "uppercase",
    },
    textUnderlineOffset: {
      ":is(*) a": "0.2em",
    },
    width: {
      ":is(*) img": "100%",
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
            {...stylex.props(styles.content)}
          />
        </article>
      </div>

      <SiteFooter />
    </main>
  );
}
