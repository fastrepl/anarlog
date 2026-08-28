import * as stylex from "@stylexjs/stylex";

import { colors, fonts, media, radii } from "@anlg/design-system/tokens.stylex";

export const authStyles = stylex.create({
  page: {
    backgroundColor: colors.card,
    color: colors.foreground,
    minHeight: "100vh",
  },
  pageContent: {
    display: "flex",
    flexDirection: "column",
    marginInline: "auto",
    maxWidth: "1180px",
    minHeight: "100vh",
    paddingInline: {
      default: "1.25rem",
      [media.sm]: "2rem",
    },
    width: "100%",
  },
  layout: {
    alignItems: "center",
    display: "grid",
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    gap: {
      default: "3rem",
      "@media (min-width: 64rem)": "5rem",
    },
    gridTemplateColumns: {
      default: null,
      [media.md]: "minmax(0, 1fr) minmax(360px, 440px)",
    },
    paddingBlock: {
      default: "2.5rem",
      "@media (min-width: 64rem)": "4rem",
    },
  },
  pitch: {
    display: {
      default: "none",
      [media.md]: "block",
    },
  },
  pitchEyebrow: {
    color: colors.mutedForeground,
    fontFamily: fonts.hand,
    fontSize: "1.5rem",
    fontWeight: 600,
    lineHeight: 1,
  },
  pitchHeading: {
    fontFamily: fonts.hand,
    fontSize: {
      default: "3.75rem",
      "@media (min-width: 64rem)": "4.5rem",
    },
    fontWeight: 600,
    letterSpacing: 0,
    lineHeight: {
      default: 0.95,
      "@media (min-width: 64rem)": 1,
    },
    marginTop: "1.25rem",
    maxWidth: "620px",
    textWrap: "balance",
  },
  pitchMark: {
    backgroundColor: "oklch(94.8% 0.067 90.6)",
    color: colors.foreground,
    paddingInline: ".25rem",
  },
  panel: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "24px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 24px 80px rgb(24 22 19 / 0.1)",
    marginInline: "auto",
    maxWidth: "440px",
    overflow: "hidden",
    width: "100%",
  },
  panelHeader: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    paddingBlock: {
      default: "1.75rem",
      [media.sm]: "2rem",
    },
    paddingInline: {
      default: "1.5rem",
      [media.sm]: "2rem",
    },
  },
  panelEyebrow: {
    color: colors.mutedForeground,
    fontFamily: fonts.hand,
    fontSize: "1.25rem",
    fontWeight: 600,
    lineHeight: 1,
  },
  panelTitle: {
    color: colors.foreground,
    fontFamily: fonts.hand,
    fontSize: "2.25rem",
    fontWeight: 600,
    lineHeight: 1,
  },
  panelTitleWithEyebrow: {
    marginTop: ".75rem",
  },
  panelDescription: {
    color: colors.mutedForeground,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    marginTop: ".75rem",
  },
  panelBody: {
    padding: {
      default: "1.5rem",
      [media.sm]: "2rem",
    },
  },
  input: {
    backgroundColor: colors.card,
    borderColor: {
      default: colors.border,
      ":hover": colors.mutedForeground,
      ":focus": colors.foreground,
    },
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: null,
      ":focus": `0 0 0 2px color-mix(in oklab, ${colors.foreground} 10%, transparent)`,
    },
    color: {
      default: colors.foreground,
      "::placeholder": colors.mutedForeground,
    },
    height: "3rem",
    outline: {
      default: null,
      ":focus": "none",
    },
    paddingInline: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "border-color, box-shadow",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  primaryButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.foreground,
      ":hover": colors.mutedForeground,
    },
    borderRadius: radii.full,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.foreground}, 0 0 0 4px ${colors.card}`,
    },
    color: colors.primaryForeground,
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "flex",
    fontSize: ".875rem",
    fontWeight: 500,
    gap: ".75rem",
    height: "3rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    paddingInline: "1.25rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  secondaryButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.card,
      ":hover": colors.muted,
    },
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.foreground}, 0 0 0 4px ${colors.card}`,
    },
    color: colors.foreground,
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "flex",
    fontSize: ".875rem",
    fontWeight: 500,
    gap: ".75rem",
    height: "3rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    paddingInline: "1.25rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  notice: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    padding: "1rem",
    textAlign: "center",
  },
});
export function AuthShell({
  title,
  description,
  showEyebrow = true,
  children,
}: {
  title: string;
  description?: string;
  showEyebrow?: boolean;
  children: React.ReactNode;
}) {
  return (
    <main {...stylex.props(authStyles.page)}>
      <div {...stylex.props(authStyles.pageContent)}>
        <div {...stylex.props(authStyles.layout)}>
          <section {...stylex.props(authStyles.pitch)}>
            <p {...stylex.props(authStyles.pitchEyebrow)}>
              Stay present. Keep the notes.
            </p>
            <h2 {...stylex.props(authStyles.pitchHeading)}>
              AI notepad for{" "}
              <mark {...stylex.props(authStyles.pitchMark)}>
                private meetings.
              </mark>
            </h2>
          </section>

          <section {...stylex.props(authStyles.panel)}>
            <header {...stylex.props(authStyles.panelHeader)}>
              {showEyebrow && (
                <p {...stylex.props(authStyles.panelEyebrow)}>
                  Private by default
                </p>
              )}
              <h1
                {...stylex.props(
                  authStyles.panelTitle,
                  showEyebrow && authStyles.panelTitleWithEyebrow,
                )}
              >
                {title}
              </h1>
              {description && (
                <p {...stylex.props(authStyles.panelDescription)}>
                  {description}
                </p>
              )}
            </header>

            <div {...stylex.props(authStyles.panelBody)}>{children}</div>
          </section>
        </div>
      </div>
    </main>
  );
}
