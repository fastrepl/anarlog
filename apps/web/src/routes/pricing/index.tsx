import { CheckCircle, XCircle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { createFileRoute, Link } from "@tanstack/react-router";

import { radii, colors, fonts } from "@anlg/design-system/tokens.stylex";
import { MARKETING_PLAN_TIERS } from "@anlg/pricing";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { EnterpriseCallout } from "@/components/enterprise-callout";
import { SiteFooter } from "@/components/site-footer";
import {
  ANARLOG_ROW,
  COMPARISON_ROWS,
  type ComparisonRow,
  PRICING_VERIFIED_ON,
} from "@/lib/competitors";
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
    maxWidth: "700px",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    paddingTop: {
      default: "1rem",
      "@media (width >= 48rem)": "1rem",
    },
    paddingBottom: {
      default: "2rem",
      "@media (width >= 48rem)": "3rem",
    },
  },
  style3: {
    minWidth: 0,
    textAlign: "center",
  },
  style4: {
    paddingTop: {
      default: "2.5rem",
      "@media (width >= 48rem)": "3rem",
    },
    paddingBottom: {
      default: "1rem",
      "@media (width >= 48rem)": "1.5rem",
    },
  },
  style5: {
    display: "inline-flex",
  },
  style6: {
    height: {
      default: "2rem",
      "@media (width >= 48rem)": "2.25rem",
    },
    width: "auto",
  },
  style7: {
    fontFamily: fonts.hand,
    marginTop: {
      default: "3rem",
      "@media (width >= 48rem)": "4rem",
    },
    fontSize: {
      default: "2.25rem",
      "@media (width >= 48rem)": "3rem",
    },
    lineHeight: {
      default: 1,
      "@media (width >= 48rem)": 1,
    },
    fontWeight: 600,
    color: "#181613",
  },
  style8: {
    marginInline: "auto",
    marginTop: "1.5rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    color: "#4f4940",
  },
  style9: {
    marginTop: "2rem",
  },
  style10: {
    display: "inline-flex",
    height: "2.75rem",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: radii.full,
    backgroundColor: {
      default: "#181613",
      ":hover": "#4f4940",
    },
    paddingInline: "1.5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#fff",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    scale: {
      default: null,
      ":hover": 1.02,
      ":active": 0.98,
    },
  },
  style11: {
    paddingTop: {
      default: "3rem",
      "@media (width >= 48rem)": "4rem",
    },
    paddingBottom: {
      default: "2rem",
      "@media (width >= 48rem)": "2.5rem",
    },
  },
  style12: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#756b5d",
  },
  style13: {
    position: "relative",
    left: "50%",
    marginTop: "2rem",
    width: "100vw",
    maxWidth: "980px",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
  },
  style14: {
    overflowX: "auto",
    borderRadius: "1.5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#eadfce",
    cornerShape: "squircle",
  },
  style15: {
    width: "100%",
    minWidth: "860px",
    borderCollapse: "collapse",
    textAlign: "left",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
  },
  style16: {
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderColor: "#eadfce",
  },
  style17: {
    marginTop: "1rem",
    textAlign: "left",
    fontSize: ".75rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style18: {
    marginTop: ".5rem",
    textAlign: "left",
    fontSize: ".75rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style19: {
    paddingTop: {
      default: "2rem",
      "@media (width >= 48rem)": "2.5rem",
    },
    paddingBottom: {
      default: "5rem",
      "@media (width >= 48rem)": "6rem",
    },
  },
  style20: {
    display: "flex",
    alignItems: "center",
    gap: ".625rem",
  },
  style21: {
    width: "1.25rem",
    height: "1.25rem",
    flexShrink: 0,
    borderRadius: "5px",
    objectFit: "contain",
  },
  style22: {
    display: "flex",
    width: "1.25rem",
    height: "1.25rem",
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: "5px",
    backgroundColor: "#f4efe6",
    fontSize: "10px",
    fontWeight: 600,
    color: "#756b5d",
  },
  style23: {
    fontWeight: 600,
    color: "#181613",
  },
  style24: {
    color: {
      default: "#4f4940",
      ":hover": "#181613",
    },
    textDecorationLine: "underline",
    textDecorationColor: "#d9cdb8",
    textUnderlineOffset: "4px",
  },
  style25: {
    paddingInline: "1rem",
    paddingBlock: ".75rem",
    whiteSpace: "nowrap",
    color: "#4f4940",
  },
  style26: {
    paddingInline: "1rem",
    paddingBlock: ".75rem",
  },
  style27: {
    display: "flex",
    justifyContent: "center",
  },
  tableHeading: {
    backgroundColor: colors.muted,
    color: colors.mutedForeground,
    fontSize: ".75rem",
    fontWeight: 600,
    paddingBlock: ".75rem",
    paddingInline: "1rem",
    whiteSpace: "nowrap",
  },
  tableHeadingCentered: {
    textAlign: "center",
  },
  tableSticky: {
    left: 0,
    position: "sticky",
    zIndex: 1,
  },
  tableRow: {
    borderBottomColor: colors.border,
    borderBottomStyle: {
      default: "solid",
      ":last-child": "none",
    },
    borderBottomWidth: {
      default: "1px",
      ":last-child": 0,
    },
  },
  highlightedRow: {
    backgroundColor: "oklch(98% 0.04 90)",
  },
  stickyCell: {
    backgroundColor: colors.card,
    left: 0,
    paddingBlock: ".75rem",
    paddingInline: "1rem",
    position: "sticky",
    whiteSpace: "nowrap",
    zIndex: 1,
  },
  highlightedCell: {
    backgroundColor: "oklch(98% 0.04 90)",
  },
  comparisonCell: {
    color: colors.mutedForeground,
    paddingBlock: ".75rem",
    paddingInline: "1rem",
    whiteSpace: "nowrap",
  },
  comparisonCellHighlighted: {
    color: colors.foreground,
    fontWeight: 600,
  },
  boolIcon: {
    height: "1.125rem",
    width: "1.125rem",
  },
  boolIconPositive: {
    color: "oklch(62.7% 0.194 149.2)",
  },
  boolIconNegative: {
    color: colors.destructive,
  },
});
const proPlan = MARKETING_PLAN_TIERS.find((plan) => plan.price);
const title = "Pricing · Anarlog";
const description =
  "Anarlog is free for unlimited local transcription, with Pro at $15/month. Compare pricing, capture method, and data ownership against Otter, Fireflies, Fathom, Granola, and other AI notetakers.";
const verifiedOnLabel = new Date(PRICING_VERIFIED_ON).toLocaleDateString(
  "en-US",
  {
    month: "long",
    day: "numeric",
    year: "numeric",
  },
);
export const Route = createFileRoute("/pricing/")({
  component: PricingPage,
  head: () => ({
    meta: [
      {
        title,
      },
      {
        name: "description",
        content: description,
      },
      {
        property: "og:title",
        content: title,
      },
      {
        property: "og:description",
        content: description,
      },
      {
        property: "og:url",
        content: getCanonicalUrl("/pricing"),
      },
      {
        name: "twitter:title",
        content: title,
      },
      {
        name: "twitter:description",
        content: description,
      },
      {
        name: "twitter:url",
        content: getCanonicalUrl("/pricing"),
      },
    ],
    links: [
      {
        rel: "canonical",
        href: getCanonicalUrl("/pricing"),
      },
    ],
  }),
});
function PricingPage() {
  return (
    <main {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <div {...stylex.props(styles.style3)}>
          <section {...stylex.props(styles.style4)}>
            <Link
              to="/"
              aria-label="Anarlog home"
              {...stylex.props(styles.style5)}
            >
              <AnarlogLogo sx={styles.style6} />
            </Link>
            <h1 {...stylex.props(styles.style7)}>Simple pricing</h1>
            <p {...stylex.props(styles.style8)}>
              Free forever for unlimited local transcription and your own API
              keys. Pro is ${proPlan?.price?.monthly}/month
              {proPlan?.price?.yearly
                ? ` or $${proPlan.price.yearly}/year`
                : null}{" "}
              when you want hosted transcription, AI, sync, and sharing.
            </p>
            <div {...stylex.props(styles.style9)}>
              <Link to="/download/" {...stylex.props(styles.style10)}>
                Download for free
              </Link>
            </div>
          </section>

          <section {...stylex.props(styles.style11)}>
            <h2 {...stylex.props(styles.style12)}>How we compare</h2>
            <p {...stylex.props(styles.style8)}>
              Most AI notetakers send your meeting to their servers and pick the
              AI for you. The column that matters is where your data lives.
            </p>

            <div {...stylex.props(styles.style13)}>
              <div {...stylex.props(styles.style14)}>
                <table {...stylex.props(styles.style15)}>
                  <thead>
                    <tr {...stylex.props(styles.style16)}>
                      <Th sticky>Tool</Th>
                      <Th>Paid from</Th>
                      <Th>Free tier</Th>
                      <Th center>Bot-free</Th>
                      <Th center>Local data</Th>
                      <Th center>Offline</Th>
                      <Th center>Local models</Th>
                      <Th center>Own keys</Th>
                      <Th center>Open source</Th>
                    </tr>
                  </thead>
                  <tbody>
                    <Row row={ANARLOG_ROW} highlight />
                    {COMPARISON_ROWS.map((row) => (
                      <Row key={row.name} row={row} />
                    ))}
                  </tbody>
                </table>
              </div>

              <p {...stylex.props(styles.style17)}>
                Bot-free means capture without adding a participant to the call.
                Local data means the meeting record is stored on your device by
                default. Offline means recording and transcription work with no
                connection.
              </p>
              <p {...stylex.props(styles.style18)}>
                Competitor details verified {verifiedOnLabel} from each vendor's
                published material, using the lowest regularly available paid
                tier. Plans change — follow the links above before deciding.
                Anarlog pricing is always current.
              </p>
            </div>
          </section>

          <section {...stylex.props(styles.style19)}>
            <EnterpriseCallout centered />
          </section>
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}
function Th({
  children,
  center = false,
  sticky = false,
}: {
  children: React.ReactNode;
  center?: boolean;
  sticky?: boolean;
}) {
  return (
    <th
      scope="col"
      {...stylex.props([
        styles.tableHeading,
        center && styles.tableHeadingCentered,
        sticky && styles.tableSticky,
      ])}
    >
      {children}
    </th>
  );
}
function Row({ row, highlight }: { row: ComparisonRow; highlight?: boolean }) {
  const isInternal = row.url.startsWith("/");
  return (
    <tr
      {...stylex.props([styles.tableRow, highlight && styles.highlightedRow])}
    >
      <td
        {...stylex.props([
          styles.stickyCell,
          highlight && styles.highlightedCell,
        ])}
      >
        <div {...stylex.props(styles.style20)}>
          {row.icon ? (
            <img
              src={row.icon}
              alt=""
              aria-hidden="true"
              {...stylex.props(styles.style21)}
            />
          ) : (
            <span aria-hidden="true" {...stylex.props(styles.style22)}>
              {row.name.charAt(0)}
            </span>
          )}
          {isInternal ? (
            <span {...stylex.props(styles.style23)}>{row.name}</span>
          ) : (
            <a
              href={row.url}
              target="_blank"
              rel="noopener noreferrer nofollow"
              {...stylex.props(styles.style24)}
            >
              {row.name}
            </a>
          )}
        </div>
      </td>
      <td
        {...stylex.props([
          styles.comparisonCell,
          highlight && styles.comparisonCellHighlighted,
        ])}
      >
        {row.paidFrom}
      </td>
      <td {...stylex.props(styles.style25)}>{row.freeTier}</td>
      <Bool value={row.botFree} />
      <Bool value={row.localData} />
      <Bool value={row.offline} />
      <Bool value={row.localModels} />
      <Bool value={row.ownKeys} />
      <Bool value={row.openSource} />
    </tr>
  );
}
function Bool({ value }: { value: boolean }) {
  const Icon = value ? CheckCircle : XCircle;
  return (
    <td {...stylex.props(styles.style26)}>
      <div {...stylex.props(styles.style27)}>
        <Icon
          {...stylex.props(
            styles.boolIcon,
            value ? styles.boolIconPositive : styles.boolIconNegative,
          )}
          aria-label={value ? "Yes" : "No"}
        />
      </div>
    </td>
  );
}
