import { ArrowRight } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { createFileRoute, Link } from "@tanstack/react-router";

import { fonts, radii } from "@anlg/design-system/tokens.stylex";

import { SiteFooter } from "@/components/site-footer";
import { changelogEntries, formatChangelogDate } from "@/lib/changelog";
import { getEntrySummary } from "@/lib/changelog-summary";
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
    borderBlockStyle: "solid",
    borderBlockWidth: "1px",
    borderColor: "#eee8df",
  },
  style9: {
    scrollMarginTop: "2rem",
    borderBottomStyle: {
      default: "solid",
      ":last-child": "solid",
    },
    borderBottomWidth: {
      default: "1px",
      ":last-child": 0,
    },
    borderColor: "#eee8df",
  },
  style10: {
    display: "grid",
    gap: {
      default: "1rem",
      "@media (width >= 40rem)": "1.5rem",
    },
    paddingBlock: {
      default: "1.75rem",
      "@media (width >= 48rem)": "2.25rem",
    },
    gridTemplateColumns: {
      default: null,
      "@media (width >= 40rem)": "10rem minmax(0, 1fr) 1.5rem",
    },
    alignItems: {
      default: null,
      "@media (width >= 40rem)": "flex-start",
    },
  },
  style11: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: ".625rem",
  },
  style12: {
    fontFamily: fonts.hand,
    fontSize: "2.25rem",
    lineHeight: 1,
    fontWeight: 600,
    letterSpacing: 0,
    color: {
      default: "#756b5d",
      [stylex.when.ancestor(":hover")]: "#181613",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style13: {
    borderRadius: radii.full,
    backgroundColor: "#f3eee6",
    paddingInline: ".5rem",
    paddingBlock: ".25rem",
    fontSize: ".65rem",
    fontWeight: 600,
    letterSpacing: ".12em",
    color: "#756b5d",
    textTransform: "uppercase",
  },
  style14: {
    marginTop: ".5rem",
    display: "block",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#756b5d",
  },
  style15: {
    fontSize: {
      default: "1rem",
      "@media (width >= 48rem)": "1.125rem",
    },
    lineHeight: {
      default: "1.75rem",
      "@media (width >= 48rem)": "2rem",
    },
    color: {
      default: "#4f4940",
      [stylex.when.ancestor(":hover")]: "#363029",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style16: {
    marginTop: ".25rem",
    display: {
      default: "none",
      "@media (width >= 40rem)": "block",
    },
    color: {
      default: "#9a9082",
      [stylex.when.ancestor(":hover")]: "#181613",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, opacity, box-shadow, transform, translate, scale, rotate, filter, -webkit-backdrop-filter, backdrop-filter, display, content-visibility, overlay, pointer-events",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    translate: {
      default: null,
      [stylex.when.ancestor(":hover")]: ".25rem 0",
    },
  },
  style17: {
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#eee8df",
    paddingTop: "2rem",
    color: "#4f4940",
  },
});
export const Route = createFileRoute("/changelog/")({
  component: Component,
  head: () => ({
    links: [
      {
        rel: "canonical",
        href: getCanonicalUrl("/changelog"),
      },
    ],
    meta: [
      {
        title: "Anarlog Changelog",
      },
      {
        name: "description",
        content:
          "See the latest Anarlog desktop app updates, fixes, and product changes.",
      },
      {
        property: "og:title",
        content: "Anarlog Changelog",
      },
      {
        property: "og:url",
        content: getCanonicalUrl("/changelog"),
      },
    ],
  }),
});
function Component() {
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
          <h1 {...stylex.props(styles.style6)}>Changelog</h1>
          <p {...stylex.props(styles.style7)}>
            Product updates, fixes, and release notes for Anarlog.
          </p>
        </section>

        {changelogEntries.length > 0 ? (
          <ol {...stylex.props(styles.style8)}>
            {changelogEntries.map((entry, index) => (
              <li
                key={entry.version}
                id={entry.version}
                {...stylex.props(styles.style9)}
              >
                <article>
                  <Link
                    to="/changelog/$version/"
                    params={{
                      version: entry.version,
                    }}
                    {...stylex.props(styles.style10, stylex.defaultMarker())}
                  >
                    <header>
                      <div {...stylex.props(styles.style11)}>
                        <h2 {...stylex.props(styles.style12)}>
                          v{entry.version}
                        </h2>
                        {index === 0 && (
                          <span {...stylex.props(styles.style13)}>Latest</span>
                        )}
                      </div>
                      {entry.date && (
                        <time
                          dateTime={entry.date}
                          {...stylex.props(styles.style14)}
                        >
                          {formatChangelogDate(entry.date)}
                        </time>
                      )}
                    </header>
                    <p {...stylex.props(styles.style15)}>
                      {getEntrySummary(entry.summary ?? entry.content)}
                    </p>
                    <ArrowRight
                      aria-hidden="true"
                      {...stylex.props(styles.style16)}
                      size={20}
                    />
                  </Link>
                </article>
              </li>
            ))}
          </ol>
        ) : (
          <p {...stylex.props(styles.style17)}>No changelog entries yet.</p>
        )}
      </div>

      <SiteFooter />
    </main>
  );
}
