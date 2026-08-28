import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";

import { colors, fonts, media, radii } from "@anlg/design-system/tokens.stylex";

const heading = "Rolling Anarlog out to a team?";
const body =
  "Workspace admin, SSO, and a self-hosted server for regulated environments — shaped with early partners.";

const styles = stylex.create({
  centeredHeading: {
    color: colors.foreground,
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    fontWeight: 600,
    lineHeight: 1,
  },
  centeredBody: {
    color: colors.mutedForeground,
    fontSize: "1rem",
    lineHeight: "1.75rem",
    marginInline: "auto",
    marginTop: "1.25rem",
    maxWidth: "32rem",
  },
  centeredAction: {
    marginTop: "2rem",
  },
  card: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: "1.5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    cornerShape: "squircle",
    display: "flex",
    flexDirection: {
      default: "column",
      [media.md]: "row",
    },
    gap: "1rem",
    justifyContent: {
      default: null,
      [media.md]: "space-between",
    },
    padding: "1.5rem",
    textAlign: "left",
  },
  cardHeading: {
    color: colors.foreground,
    fontSize: ".875rem",
    fontWeight: 600,
  },
  cardBody: {
    color: colors.mutedForeground,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    marginTop: ".25rem",
  },
  cta: {
    alignItems: "center",
    backgroundColor: {
      default: colors.foreground,
      ":hover": colors.mutedForeground,
    },
    borderRadius: radii.full,
    color: colors.primaryForeground,
    display: "inline-flex",
    flexShrink: 0,
    fontSize: ".875rem",
    fontWeight: 500,
    height: "2.75rem",
    justifyContent: "center",
    paddingInline: "1.5rem",
    scale: {
      default: 1,
      ":hover": 1.02,
      ":active": 0.98,
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
});

// `centered` drops the card treatment for pages that have no other cards to
// sit alongside.
export function EnterpriseCallout({
  centered = false,
}: {
  centered?: boolean;
}) {
  if (centered) {
    return (
      <div>
        <h2 {...stylex.props(styles.centeredHeading)}>{heading}</h2>
        <p {...stylex.props(styles.centeredBody)}>{body}</p>
        <div {...stylex.props(styles.centeredAction)}>
          <Link to="/enterprise/" {...stylex.props(styles.cta)}>
            Talk to sales
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.card)}>
      <div>
        <p {...stylex.props(styles.cardHeading)}>{heading}</p>
        <p {...stylex.props(styles.cardBody)}>{body}</p>
      </div>
      <Link to="/enterprise/" {...stylex.props(styles.cta)}>
        Talk to sales
      </Link>
    </div>
  );
}
