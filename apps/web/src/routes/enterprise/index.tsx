import { BellRinging, CheckCircle, HardDrives } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { createFileRoute, Link } from "@tanstack/react-router";

import { fonts, media, radii } from "@anlg/design-system/tokens.stylex";
import type { StyleXProps } from "@anlg/ui/lib/stylex";

import { AnarlogLogo } from "@/components/anarlog-logo";
import {
  LocalFilesVisual,
  MeetingCaptureVisual,
} from "@/components/home-page/privacy-section";
import { SiteFooter } from "@/components/site-footer";
import { BOOK_CALL_URL } from "@/lib/enterprise";
import { getCanonicalUrl } from "@/lib/seo";

const partnerHandLeft = stylex.keyframes({
  "0%": {
    transform: "translateX(-3.5rem) rotate(-9deg)",
  },
  "55%": {
    transform: "translateX(0) rotate(0deg)",
  },
  "70%": {
    transform: "translateX(0) translateY(-0.2rem) rotate(-1.5deg)",
  },
  "85%": {
    transform: "translateX(0) translateY(0.15rem) rotate(1deg)",
  },
  "100%": {
    transform: "translateX(0) translateY(0) rotate(0deg)",
  },
});

const partnerHandRight = stylex.keyframes({
  "0%": {
    transform: "translateX(3.5rem) rotate(9deg)",
  },
  "55%": {
    transform: "translateX(0) rotate(0deg)",
  },
  "70%": {
    transform: "translateX(0) translateY(0.2rem) rotate(1.5deg)",
  },
  "85%": {
    transform: "translateX(0) translateY(-0.15rem) rotate(-1deg)",
  },
  "100%": {
    transform: "translateX(0) translateY(0) rotate(0deg)",
  },
});

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
    marginTop: {
      default: "3rem",
      "@media (width >= 48rem)": "4rem",
    },
    fontFamily: fonts.hand,
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
    marginTop: ".75rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#756b5d",
  },
  style11: {
    paddingTop: {
      default: "3rem",
      "@media (width >= 48rem)": "4rem",
    },
    paddingBottom: {
      default: "1rem",
      "@media (width >= 48rem)": "1.5rem",
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
    marginTop: "1.5rem",
    width: "100vw",
    maxWidth: "1120px",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
  },
  style14: {
    display: "flex",
    flexDirection: "column",
    gap: {
      default: "1rem",
      "@media (width >= 48rem)": "2rem",
    },
  },
  style15: {
    display: "flex",
    flexDirection: "column",
    paddingInline: "1.5rem",
    paddingBlock: ".75rem",
    textAlign: "center",
    width: {
      default: null,
      "@media (width >= 48rem)": "31%",
    },
    padding: {
      default: null,
      "@media (width >= 48rem)": "1rem",
    },
  },
  style16: {
    marginTop: {
      default: "1.25rem",
      "@media (width >= 48rem)": "1.75rem",
    },
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
    color: "#4f4940",
  },
  style17: {
    marginInline: "auto",
    marginTop: ".25rem",
    maxWidth: "17rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#4f4940",
  },
  style18: {
    paddingTop: {
      default: "3rem",
      "@media (width >= 48rem)": "3.5rem",
    },
    paddingBottom: {
      default: "1rem",
      "@media (width >= 48rem)": "1.5rem",
    },
  },
  style19: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    paddingBottom: "1rem",
    userSelect: "none",
  },
  style20: {
    animationFillMode: "both",
    animationName: {
      default: partnerHandLeft,
      [media.reducedMotion]: "none",
    },
    animationTimeline: "view()",
    animationTimingFunction: "linear",
    display: "inline-flex",
  },
  style21: {
    height: {
      default: "3rem",
      "@media (width >= 48rem)": "3.5rem",
    },
    width: "auto",
  },
  style22: {
    animationFillMode: "both",
    animationName: {
      default: partnerHandRight,
      [media.reducedMotion]: "none",
    },
    animationTimeline: "view()",
    animationTimingFunction: "linear",
    marginTop: ".625rem",
    marginLeft: {
      default: "-3rem",
      "@media (width >= 48rem)": "-3.5rem",
    },
    display: "inline-flex",
  },
  style23: {
    height: {
      default: "3rem",
      "@media (width >= 48rem)": "3.5rem",
    },
    width: "auto",
    scale: "calc(100% * -1) 1",
  },
  style24: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#181613",
  },
  style25: {
    marginInline: "auto",
    marginTop: "1.25rem",
    fontSize: "1rem",
    lineHeight: "1.75rem",
    color: "#4f4940",
  },
  style26: {
    paddingTop: {
      default: "2rem",
      "@media (width >= 48rem)": "2.5rem",
    },
    paddingBottom: {
      default: "5rem",
      "@media (width >= 48rem)": "6rem",
    },
  },
  style27: {
    marginInline: "auto",
    marginTop: "1.25rem",
    fontSize: "1rem",
    lineHeight: "1.75rem",
    color: "#4f4940",
  },
  style28: {
    marginTop: "2rem",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    gap: "1rem",
  },
  style29: {
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
  style30: {
    display: "flex",
    height: {
      default: "5rem",
      "@media (width >= 48rem)": "7rem",
    },
    alignItems: "center",
    justifyContent: "center",
    userSelect: "none",
    width: {
      default: null,
      "@media (width >= 48rem)": "100%",
    },
  },
  style31: {
    display: "flex",
    width: "100%",
    maxWidth: "260px",
    alignItems: "center",
    gap: ".75rem",
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#e5e5e5",
    backgroundColor: "#fff",
    paddingBlock: ".5rem",
    paddingRight: ".75rem",
    paddingLeft: "1rem",
    textAlign: "left",
    boxShadow: "0 3px 10px #1816130a",
  },
  style32: {
    color: "#44403c",
  },
  style33: {
    display: "flex",
    flexDirection: "column",
    gap: ".25rem",
  },
  style34: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#292524",
  },
  style35: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#a8a29e",
  },
  style36: {
    marginLeft: "auto",
    color: "#10b981",
  },
  style37: {
    display: "flex",
  },
  overlappingAvatar: {
    marginLeft: "-.625rem",
  },
  style38: {
    display: "flex",
    height: "1.75rem",
    width: "1.75rem",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "2px",
    borderColor: "#fff",
    backgroundColor: "#eadfce",
    fontSize: ".75rem",
    lineHeight: "1rem",
    fontWeight: 600,
    color: "#756b5d",
    cornerShape: "round",
  },
  style39: {
    marginLeft: "auto",
    height: ".625rem",
    width: ".625rem",
    borderRadius: radii.full,
    backgroundColor: "#10b981",
    cornerShape: "round",
  },
  style40: {
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
  pillarRow: {
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "flex-start",
    },
    display: {
      default: "grid",
      "@media (width >= 48rem)": "flex",
    },
    gap: {
      default: "1rem",
      "@media (width >= 48rem)": 0,
    },
  },
  threePillarRow: {
    justifyContent: {
      default: null,
      "@media (width >= 48rem)": "space-between",
    },
  },
  twoPillarRow: {
    justifyContent: {
      default: null,
      "@media (width >= 48rem)": "space-evenly",
    },
  },
});
const title = "Enterprise · Anarlog";
const description =
  "Anarlog for teams and enterprises: end-to-end encrypted meeting notes with no meeting bots, workspace admin controls, and a self-hostable server. Book a call with the founder.";
export const Route = createFileRoute("/enterprise/")({
  component: EnterprisePage,
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
        content: getCanonicalUrl("/enterprise"),
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
        content: getCanonicalUrl("/enterprise"),
      },
    ],
    links: [
      {
        rel: "canonical",
        href: getCanonicalUrl("/enterprise"),
      },
    ],
  }),
});
const pillarRows = [
  [
    {
      title: "Private by architecture",
      body: "Notes live in local SQLite, and cloud sync is end-to-end encrypted — our servers only ever hold ciphertext.",
      Visual: LocalFilesVisual,
    },
    {
      title: "No bots in your meetings",
      body: "Anarlog listens locally. Nothing joins your calls, and nothing appears in participant lists.",
      Visual: MeetingCaptureVisual,
    },
    {
      title: "Consent on your terms",
      body: "Recording disclosure and consent defaults set once, org-wide — every meeting meets the same bar.",
      Visual: ConsentNoticeVisual,
    },
  ],
  [
    {
      title: "Admin without surveillance",
      body: "Members, roles, seats, and org-wide policies built on metadata — never on anyone's notes.",
      Visual: WorkspaceAdminVisual,
    },
    {
      title: "Self-host the whole stack",
      body: "Run the Anarlog server on infrastructure you control for regulated environments.",
      Visual: SelfHostVisual,
    },
  ],
];
function EnterprisePage() {
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
            <h1 {...stylex.props(styles.style7)}>
              Meeting memory your company owns
            </h1>
            <p {...stylex.props(styles.style8)}>
              Bring Anarlog to your whole team without handing your
              conversations to another cloud. Notes stay on your machines, sync
              is end-to-end encrypted, and no bot ever joins a call.
            </p>
            <div {...stylex.props(styles.style9)}>
              <BookCallButton />
            </div>
            <p {...stylex.props(styles.style10)}>
              30 minutes, directly with the founder. No SDR queue.
            </p>
          </section>

          <section {...stylex.props(styles.style11)}>
            <h2 {...stylex.props(styles.style12)}>Why teams pick Anarlog</h2>
            <div {...stylex.props(styles.style13)}>
              <div {...stylex.props(styles.style14)}>
                {pillarRows.map((row) => (
                  <div
                    key={row[0].title}
                    {...stylex.props([
                      styles.pillarRow,
                      row.length === 3
                        ? styles.threePillarRow
                        : styles.twoPillarRow,
                    ])}
                  >
                    {row.map((pillar) => (
                      <div key={pillar.title} {...stylex.props(styles.style15)}>
                        <pillar.Visual />
                        <h3 {...stylex.props(styles.style16)}>
                          {pillar.title}
                        </h3>
                        <p {...stylex.props(styles.style17)}>{pillar.body}</p>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          </section>

          <section {...stylex.props(styles.style18)}>
            <div {...stylex.props(styles.style19)} aria-hidden="true">
              <span {...stylex.props(styles.style20)}>
                <PartnerHandSvg sleeve="#181613" sx={styles.style21} />
              </span>
              <span {...stylex.props(styles.style22)}>
                <PartnerHandSvg sleeve="#eadfce" sx={styles.style23} />
              </span>
            </div>
            <h2 {...stylex.props(styles.style24)}>Built with early partners</h2>
            <p {...stylex.props(styles.style25)}>
              Team workspaces with admin controls, SSO and SCIM, and the
              self-hosted server are in active development. Early enterprise
              partners work directly with the founding team and shape what ships
              first.
            </p>
          </section>

          <section {...stylex.props(styles.style26)}>
            <h2 {...stylex.props(styles.style24)}>Talk to us</h2>
            <p {...stylex.props(styles.style27)}>
              Tell us about your team and your compliance needs — we'll show you
              what works today and what lands next.
            </p>
            <div {...stylex.props(styles.style28)}>
              <BookCallButton />
              <Link to="/pricing/" {...stylex.props(styles.style29)}>
                Compare plans and pricing
              </Link>
            </div>
          </section>
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}
function PartnerHandSvg({
  sleeve,
  sx,
}: {
  sleeve: string;
} & StyleXProps) {
  return (
    <svg
      viewBox="0 0 128 60"
      fill="none"
      {...stylex.props(sx)}
      aria-hidden="true"
    >
      <path
        d="M30 20 H78 C98 20 114 27 117 38 C119 47 108 52 92 51 L38 51 C32 51 28 46 28 40 Z"
        fill="#fffaf0"
        stroke="#181613"
        strokeWidth="2.5"
        strokeLinejoin="round"
      />
      <path
        d="M82 22 C88 12 102 13 106 21 C109 27 103 31 95 30"
        fill="#fffaf0"
        stroke="#181613"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M104 33 C109 34 113 37 115 41 M96 48 C101 48 106 47 110 45"
        stroke="#181613"
        strokeWidth="2"
        strokeLinecap="round"
      />
      <rect
        x="2"
        y="12"
        width="26"
        height="42"
        rx="7"
        fill={sleeve}
        stroke="#181613"
        strokeWidth="2.5"
      />
    </svg>
  );
}
function ConsentNoticeVisual() {
  return (
    <div {...stylex.props(styles.style30)}>
      <div {...stylex.props(styles.style31)}>
        <BellRinging
          size={28}
          {...stylex.props(styles.style32)}
          aria-hidden="true"
        />
        <div {...stylex.props(styles.style33)}>
          <span {...stylex.props(styles.style34)}>Consent notice sent</span>
          <span {...stylex.props(styles.style35)}>org-wide policy</span>
        </div>
        <CheckCircle
          size={20}
          weight="fill"
          {...stylex.props(styles.style36)}
          aria-hidden="true"
        />
      </div>
    </div>
  );
}
function WorkspaceAdminVisual() {
  return (
    <div {...stylex.props(styles.style30)}>
      <div {...stylex.props(styles.style31)}>
        <div {...stylex.props(styles.style37)} aria-hidden="true">
          {["S", "B", "A"].map((initial, index) => (
            <span
              key={initial}
              {...stylex.props(
                styles.style38,
                index > 0 && styles.overlappingAvatar,
              )}
            >
              {initial}
            </span>
          ))}
        </div>
        <div {...stylex.props(styles.style33)}>
          <span {...stylex.props(styles.style34)}>Design team</span>
          <span {...stylex.props(styles.style35)}>12 seats</span>
        </div>
      </div>
    </div>
  );
}
function SelfHostVisual() {
  return (
    <div {...stylex.props(styles.style30)}>
      <div {...stylex.props(styles.style31)}>
        <HardDrives
          size={28}
          {...stylex.props(styles.style32)}
          aria-hidden="true"
        />
        <div {...stylex.props(styles.style33)}>
          <span {...stylex.props(styles.style34)}>notes.acme.internal</span>
          <span {...stylex.props(styles.style35)}>your infrastructure</span>
        </div>
        <span {...stylex.props(styles.style39)} aria-hidden="true" />
      </div>
    </div>
  );
}
function BookCallButton() {
  return (
    <a
      href={BOOK_CALL_URL}
      target="_blank"
      rel="noopener noreferrer"
      {...stylex.props(styles.style40)}
    >
      Book a call with the founder
    </a>
  );
}
