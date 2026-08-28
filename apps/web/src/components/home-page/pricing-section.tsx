import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import {
  MARKETING_PLAN_TIERS,
  type MarketingPlanData,
  PlanFeatureList,
} from "@anlg/pricing";

import { EnterpriseCallout } from "@/components/enterprise-callout";
const styles = stylex.create({
  style1: {
    paddingTop: {
      default: "6rem",
      "@media (width >= 48rem)": "7rem",
    },
    paddingBottom: {
      default: "2rem",
      "@media (width >= 48rem)": "2.5rem",
    },
  },
  style2: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#756b5d",
  },
  style3: {
    marginInline: "auto",
    marginTop: "1.5rem",
    maxWidth: "42rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    color: "#4f4940",
  },
  style4: {
    position: "relative",
    left: "50%",
    marginTop: "2rem",
    display: "grid",
    width: "100vw",
    maxWidth: "760px",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    gridTemplateColumns: {
      default: "repeat(1, minmax(0, 1fr))",
      "@media (width >= 48rem)": "repeat(2, minmax(0, 1fr))",
    },
    gap: "1rem",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    textAlign: "left",
  },
  style5: {
    position: "relative",
    left: "50%",
    marginTop: "1.5rem",
    width: "100vw",
    maxWidth: "760px",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
  },
  style6: {
    marginTop: "2rem",
  },
  style7: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
    textDecorationLine: "underline",
    textDecorationColor: "#d9cdb8",
    textUnderlineOffset: "4px",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style8: {
    display: "flex",
    alignItems: "flex-start",
  },
  style9: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#181613",
  },
  style10: {
    marginTop: "1rem",
    minHeight: "4.5rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#4f4940",
  },
  style11: {
    marginTop: "1.25rem",
    minHeight: "4rem",
  },
  style12: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "baseline",
    columnGap: ".5rem",
    rowGap: ".25rem",
  },
  style13: {
    fontFamily: fonts.hand,
    fontSize: "3rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#181613",
  },
  style14: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style15: {
    display: "flex",
    alignItems: "baseline",
    gap: ".5rem",
  },
  style16: {
    marginTop: "1.25rem",
  },
  style17: {
    marginTop: "auto",
    paddingTop: "1.5rem",
  },
  pricingCard: {
    backgroundColor: colors.card,
    borderRadius: "1.5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    cornerShape: "squircle",
    display: "flex",
    flexDirection: "column",
    minHeight: "30rem",
    padding: "1.5rem",
    transitionDuration: "200ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  popularCard: {
    borderColor: `color-mix(in oklab, ${colors.foreground} 30%, transparent)`,
    boxShadow:
      "0 0 0 1px rgb(24 22 19 / 0.1), 0 22px 60px rgb(24 22 19 / 0.14)",
  },
  standardCard: {
    borderColor: colors.border,
    boxShadow: {
      default: "0 10px 32px rgb(24 22 19 / 0.05)",
      ":hover": "0 16px 46px rgb(24 22 19 / 0.08)",
      ":focus-within": "0 16px 46px rgb(24 22 19 / 0.08)",
    },
    opacity: {
      default: 0.58,
      ":hover": 1,
      ":focus-within": 1,
    },
  },
  pricingCta: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "flex",
    fontSize: ".875rem",
    fontWeight: 500,
    height: "2.75rem",
    justifyContent: "center",
    scale: {
      default: 1,
      ":hover": 1.02,
      ":active": 0.98,
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  pricingCtaPopular: {
    backgroundColor: {
      default: colors.foreground,
      ":hover": colors.mutedForeground,
    },
    color: colors.primaryForeground,
  },
  pricingCtaStandard: {
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.border,
    },
    color: colors.foreground,
  },
});
export function PricingSection({
  compareLink = false,
}: {
  compareLink?: boolean;
}) {
  return (
    <section id="pricing" {...stylex.props(styles.style1)}>
      <div>
        <h2 {...stylex.props(styles.style2)}>Simple pricing</h2>
        <p {...stylex.props(styles.style3)}>
          Start with local meeting notes for free. Upgrade when you want hosted
          transcription, AI, sync, and sharing.
        </p>
      </div>

      <div {...stylex.props(styles.style4)}>
        {MARKETING_PLAN_TIERS.map((plan) => (
          <PricingCard key={plan.id} plan={plan} />
        ))}
      </div>

      <div {...stylex.props(styles.style5)}>
        <EnterpriseCallout />
      </div>

      {compareLink ? (
        <div {...stylex.props(styles.style6)}>
          <Link to="/pricing/" {...stylex.props(styles.style7)}>
            Compare with other AI notetakers
          </Link>
        </div>
      ) : null}
    </section>
  );
}
function PricingCard({ plan }: { plan: MarketingPlanData }) {
  const visibleFeatures = plan.features.filter((feature) => feature.included);
  return (
    <article
      {...stylex.props([
        styles.pricingCard,
        plan.popular ? styles.popularCard : styles.standardCard,
      ])}
    >
      <div {...stylex.props(styles.style8)}>
        <h3 {...stylex.props(styles.style9)}>{plan.name}</h3>
      </div>

      <p {...stylex.props(styles.style10)}>{plan.description}</p>

      <div {...stylex.props(styles.style11)}>
        {plan.price ? (
          <div {...stylex.props(styles.style12)}>
            <span {...stylex.props(styles.style13)}>${plan.price.monthly}</span>
            <span {...stylex.props(styles.style14)}>/month</span>
            {plan.price.yearly != null ? (
              <span {...stylex.props(styles.style14)}>
                or ${plan.price.yearly}/year
              </span>
            ) : null}
          </div>
        ) : (
          <div {...stylex.props(styles.style15)}>
            <span {...stylex.props(styles.style13)}>$0</span>
            <span {...stylex.props(styles.style14)}>/month</span>
          </div>
        )}
      </div>

      <div {...stylex.props(styles.style16)}>
        <PlanFeatureList features={visibleFeatures} dense />
      </div>

      <div {...stylex.props(styles.style17)}>
        <Link
          to="/download/"
          {...stylex.props(
            styles.pricingCta,
            plan.popular ? styles.pricingCtaPopular : styles.pricingCtaStandard,
          )}
        >
          {plan.price ? "Start your 3-week Pro trial" : "Download for free"}
        </Link>
      </div>
    </article>
  );
}
