import {
  Cloud,
  Cpu,
  type Icon as PhosphorIcon,
  Key,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, fonts, media } from "@anlg/design-system/tokens.stylex";
import type { StyleXProps } from "@anlg/ui/lib/stylex";

const cardShuffleCloud = stylex.keyframes({
  "0%, 26%, 100%": {
    transform: "translate(1.3rem, 0.35rem) rotate(8deg) scale(1)",
    zIndex: 3,
  },
  "36%, 60%": {
    transform: "translate(-1.4rem, 0.7rem) rotate(-14deg) scale(0.92)",
    zIndex: 1,
  },
  "70%, 94%": {
    transform: "translate(-0.1rem, 0.2rem) rotate(-5deg) scale(0.96)",
    zIndex: 2,
  },
});

const cardShuffleKey = stylex.keyframes({
  "0%, 26%, 100%": {
    transform: "translate(-0.1rem, 0.2rem) rotate(-5deg) scale(0.96)",
    zIndex: 2,
  },
  "36%, 60%": {
    transform: "translate(1.3rem, 0.35rem) rotate(8deg) scale(1)",
    zIndex: 3,
  },
  "70%, 94%": {
    transform: "translate(-1.4rem, 0.7rem) rotate(-14deg) scale(0.92)",
    zIndex: 1,
  },
});

const cardShuffleChip = stylex.keyframes({
  "0%, 26%, 100%": {
    transform: "translate(-1.4rem, 0.7rem) rotate(-14deg) scale(0.92)",
    zIndex: 1,
  },
  "36%, 60%": {
    transform: "translate(-0.1rem, 0.2rem) rotate(-5deg) scale(0.96)",
    zIndex: 2,
  },
  "70%, 94%": {
    transform: "translate(1.3rem, 0.35rem) rotate(8deg) scale(1)",
    zIndex: 3,
  },
});

const audioBar = stylex.keyframes({
  "0%, 100%": { transform: "scaleY(0.58)" },
  "50%": { transform: "scaleY(0.86)" },
});

const audioBarMiddle = stylex.keyframes({
  "0%, 100%": { transform: "scaleY(0.9)" },
  "50%": { transform: "scaleY(1.16)" },
});

const styles = stylex.create({
  style1: {
    paddingBlock: {
      default: "4rem",
      "@media (width >= 48rem)": "5rem",
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
    marginTop: "1.5rem",
    width: "100vw",
    maxWidth: "1120px",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
  },
  style5: {
    display: {
      default: "grid",
      "@media (width >= 48rem)": "flex",
    },
    gap: {
      default: "1rem",
      "@media (width >= 48rem)": 0,
    },
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "flex-start",
    },
    justifyContent: {
      default: null,
      "@media (width >= 48rem)": "space-between",
    },
  },
  style6: {
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
  style7: {
    marginTop: {
      default: "1.25rem",
      "@media (width >= 48rem)": "1.75rem",
    },
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
    color: "#4f4940",
  },
  style8: {
    marginInline: "auto",
    marginTop: ".25rem",
    maxWidth: "17rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#4f4940",
  },
  style9: {
    display: "flex",
    height: {
      default: "6rem",
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
  style10: {
    position: "relative",
    height: {
      default: "5rem",
      "@media (width >= 48rem)": "6rem",
    },
    width: {
      default: "11rem",
      "@media (width >= 48rem)": "12rem",
    },
  },
  style11: {
    animationName: {
      default: cardShuffleCloud,
      [media.reducedMotion]: "none",
    },
    color: colors.destructive,
    transform: {
      default: null,
      [media.reducedMotion]: "translate(1.3rem, 0.35rem) rotate(8deg) scale(1)",
    },
    zIndex: {
      default: null,
      [media.reducedMotion]: 3,
    },
  },
  style12: {
    animationName: {
      default: cardShuffleKey,
      [media.reducedMotion]: "none",
    },
    transform: {
      default: null,
      [media.reducedMotion]:
        "translate(-0.1rem, 0.2rem) rotate(-5deg) scale(0.96)",
    },
    zIndex: {
      default: null,
      [media.reducedMotion]: 2,
    },
  },
  style13: {
    animationName: {
      default: cardShuffleChip,
      [media.reducedMotion]: "none",
    },
    transform: {
      default: null,
      [media.reducedMotion]:
        "translate(-1.4rem, 0.7rem) rotate(-14deg) scale(0.92)",
    },
    zIndex: {
      default: null,
      [media.reducedMotion]: 1,
    },
  },
  style14: {
    display: "flex",
    height: {
      default: "5rem",
      "@media (width >= 48rem)": "7rem",
    },
    alignItems: "center",
    justifyContent: {
      default: "center",
      "@media (width >= 48rem)": "space-between",
    },
    gap: {
      default: ".5rem",
      "@media (width >= 48rem)": ".25rem",
    },
    userSelect: "none",
    width: {
      default: null,
      "@media (width >= 48rem)": "100%",
    },
  },
  style15: {
    width: "2.5rem",
    rotate: {
      default: "3deg",
      ":hover": "7deg",
    },
    objectFit: "contain",
    transitionProperty: "transform, translate, scale, rotate",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".3s",
  },
  style16: {
    width: "2.5rem",
    rotate: {
      default: "-5deg",
      ":hover": "-9deg",
    },
    objectFit: "contain",
    transitionProperty: "transform, translate, scale, rotate",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".3s",
  },
  style17: {
    width: "3.5rem",
    objectFit: "contain",
    transitionProperty: "transform, translate, scale, rotate",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".3s",
    rotate: {
      default: null,
      ":hover": "3deg",
    },
  },
  style18: {
    width: "2.5rem",
    rotate: {
      default: "6deg",
      ":hover": "10deg",
    },
    objectFit: "contain",
    transitionProperty: "transform, translate, scale, rotate",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".3s",
  },
  style19: {
    width: "2.5rem",
    rotate: {
      default: "-4deg",
      ":hover": "-8deg",
    },
    objectFit: "contain",
    transitionProperty: "transform, translate, scale, rotate",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".3s",
  },
  style20: {
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
  style21: {
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
  style22: {
    height: "1.75rem",
    width: "1.75rem",
    objectFit: "contain",
  },
  style23: {
    display: "flex",
    flexDirection: "column",
    gap: ".25rem",
  },
  style24: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#292524",
  },
  style25: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#a8a29e",
  },
  style26: {
    marginLeft: "auto",
    display: "flex",
    height: "1.5rem",
    alignItems: "center",
    gap: ".125rem",
  },
  style27: {
    animationDuration: ".9s",
    animationIterationCount: "infinite",
    animationName: {
      default: audioBar,
      [media.reducedMotion]: "none",
    },
    animationTimingFunction: "ease-in-out",
    backgroundColor: colors.mutedForeground,
    borderRadius: "999px",
    display: "block",
    height: ".875rem",
    transform: {
      default: null,
      [media.reducedMotion]: "scaleY(0.7)",
    },
    transformOrigin: "center",
    width: ".25rem",
  },
  audioBarMiddle: {
    animationDelay: ".14s",
    animationName: {
      default: audioBarMiddle,
      [media.reducedMotion]: "none",
    },
    height: "1.125rem",
  },
  audioBarLast: {
    animationDelay: ".28s",
  },
  playingCard: {
    alignItems: "center",
    animationDuration: "9s",
    animationIterationCount: "infinite",
    animationTimingFunction: "ease-in-out",
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: ".35rem",
    borderStyle: "solid",
    borderWidth: "1.5px",
    boxShadow: "0 10px 18px rgb(24 22 19 / 0.14)",
    color: colors.foreground,
    display: "flex",
    height: "4.6rem",
    justifyContent: "center",
    left: "50%",
    marginLeft: "-1.7rem",
    overflow: "hidden",
    position: "absolute",
    top: ".2rem",
    transformOrigin: "center",
    width: "3.4rem",
    willChange: "transform",
  },
  style28: {
    alignItems: "center",
    display: "flex",
    fontSize: ".7rem",
    fontWeight: 700,
    left: ".35rem",
    letterSpacing: 0,
    lineHeight: 0.86,
    position: "absolute",
    top: ".35rem",
  },
  style29: {
    fontFamily: "Georgia, 'Times New Roman', serif",
    fontWeight: 700,
  },
  style30: {
    alignItems: "center",
    display: "flex",
    height: "1.9rem",
    justifyContent: "center",
    width: "1.9rem",
  },
  cardFaceIcon: {
    height: "1.6rem",
    strokeWidth: 2.4,
    width: "1.6rem",
  },
  style31: {
    alignItems: "center",
    bottom: ".35rem",
    display: "flex",
    fontSize: ".7rem",
    fontWeight: 700,
    letterSpacing: 0,
    lineHeight: 0.86,
    position: "absolute",
    right: ".35rem",
    transform: "rotate(180deg)",
  },
});
const privacyCommitments = [
  {
    title: "Invisible while you meet",
    description:
      "No bot joins the meeting, and Anarlog stays hidden while sharing screens.",
    visual: "meeting",
  },
  {
    title: "Local by default",
    description:
      "Your notes, transcripts, attachments, and recordings stay on your device by default.",
    visual: "files",
  },
  {
    title: "Own your AI stack",
    description:
      "Run models on your device, bring your own keys, or use our AI.",
    visual: "key",
  },
];
export function PrivacySection() {
  return (
    <section {...stylex.props(styles.style1)}>
      <div>
        <h2 {...stylex.props(styles.style2)}>Private from call to file</h2>
        <p {...stylex.props(styles.style3)}>
          Anarlog works quietly from your desktop, without joining your calls,
          forcing your data into the cloud, or locking it inside our app.
        </p>
      </div>

      <div {...stylex.props(styles.style4)}>
        <div {...stylex.props(styles.style5)}>
          {privacyCommitments.map((commitment) => {
            return (
              <div
                key={commitment.description}
                {...stylex.props(styles.style6)}
              >
                <PrivacyVisual type={commitment.visual} />
                <h3 {...stylex.props(styles.style7)}>{commitment.title}</h3>
                <p {...stylex.props(styles.style8)}>{commitment.description}</p>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
function PrivacyVisual({
  type,
}: {
  type: (typeof privacyCommitments)[number]["visual"];
}) {
  if (type === "files") {
    return <LocalFilesVisual />;
  }
  if (type === "key") {
    return (
      <div {...stylex.props(styles.style9)}>
        <div
          {...stylex.props(styles.style10)}
          role="img"
          aria-label="AI option cards cycling between cloud, key, and chip"
        >
          <AiOptionPlayingCard
            sx={styles.style11}
            rank="C"
            IconComponent={Cloud}
          />
          <AiOptionPlayingCard
            sx={styles.style12}
            rank="K"
            IconComponent={Key}
          />
          <AiOptionPlayingCard
            sx={styles.style13}
            rank="O"
            IconComponent={Cpu}
          />
        </div>
      </div>
    );
  }
  return <MeetingCaptureVisual />;
}
export function LocalFilesVisual() {
  return (
    <div {...stylex.props(styles.style14)}>
      <img
        src="/icons/file.webp"
        alt=""
        {...stylex.props(styles.style15)}
        draggable={false}
      />
      <img
        src="/icons/file.webp"
        alt=""
        width={160}
        height={221}
        {...stylex.props(styles.style16)}
        draggable={false}
      />
      <img
        src="/icons/folderchar.svg"
        alt=""
        width={1004}
        height={841}
        {...stylex.props(styles.style17)}
        draggable={false}
      />
      <img
        src="/icons/file.webp"
        alt=""
        width={160}
        height={221}
        {...stylex.props(styles.style18)}
        draggable={false}
      />
      <img
        src="/icons/file.webp"
        alt=""
        {...stylex.props(styles.style19)}
        draggable={false}
      />
    </div>
  );
}
export function MeetingCaptureVisual() {
  return (
    <div {...stylex.props(styles.style20)}>
      <div {...stylex.props(styles.style21)}>
        <img
          src="/icons/google-meet.svg"
          alt=""
          {...stylex.props(styles.style22)}
          draggable={false}
        />
        <div {...stylex.props(styles.style23)}>
          <span {...stylex.props(styles.style24)}>Sprint 3 planning</span>
          <span {...stylex.props(styles.style25)}>5 participants</span>
        </div>
        <div {...stylex.props(styles.style26)} aria-hidden="true">
          <span {...stylex.props(styles.style27)} />
          <span {...stylex.props(styles.style27, styles.audioBarMiddle)} />
          <span {...stylex.props(styles.style27, styles.audioBarLast)} />
        </div>
      </div>
    </div>
  );
}
function AiOptionPlayingCard({
  rank,
  IconComponent,
  sx,
}: {
  rank: string;
  IconComponent: PhosphorIcon;
} & StyleXProps) {
  return (
    <div {...stylex.props(styles.playingCard, sx)}>
      <span {...stylex.props(styles.style28)}>
        <span {...stylex.props(styles.style29)}>{rank}</span>
      </span>
      <div {...stylex.props(styles.style30)}>
        <IconComponent
          {...stylex.props(styles.cardFaceIcon)}
          aria-hidden="true"
        />
      </div>
      <span {...stylex.props(styles.style31)}>
        <span {...stylex.props(styles.style29)}>{rank}</span>
      </span>
    </div>
  );
}
