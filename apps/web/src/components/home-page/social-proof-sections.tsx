import { ArrowRight, XLogo } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { type CSSProperties, useState } from "react";

import { colors, fonts, media, radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import { getResizedImageSrcSet, getResizedImageUrl } from "@/lib/image-cdn";

// `width`/`height` are the intrinsic dimensions of each asset so the browser
// can reserve the correct aspect ratio before the image loads (CLS). CSS still
// controls the rendered size. `resizeWidth` routes oversized bitmap logos
// through the Netlify Image CDN at ~2x their rendered width.
const scrollLeft = stylex.keyframes({
  from: { transform: "translateX(0)" },
  to: { transform: "translateX(-50%)" },
});

const styles = stylex.create({
  style1: {
    backgroundColor: "#fff0b3",
    boxDecorationBreak: "clone",
    borderRadius: ".125rem",
    paddingInline: ".25rem",
    paddingBlock: ".125rem",
    color: "#181613",
  },
  style2: {
    display: "flex",
    height: "100%",
    flexDirection: "column",
  },
  style3: {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: "1rem",
  },
  style4: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: ".75rem",
  },
  style5: {
    width: "3rem",
    height: "3rem",
    borderRadius: radii.full,
    objectFit: "cover",
    boxShadow: "0 1px 2px 0 #0000000d",
  },
  style6: {
    minWidth: 0,
  },
  style7: {
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 600,
    color: "#181613",
  },
  style8: {
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style9: {
    display: "inline-flex",
    width: "2.25rem",
    height: "2.25rem",
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: radii.full,
    color: "#181613",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    backgroundColor: {
      default: null,
      ":hover": "#f7f4ef",
    },
  },
  style10: {
    display: "flex",
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    alignItems: "center",
    justifyContent: "flex-start",
    paddingBlock: ".75rem",
  },
  style11: {
    textAlign: "left",
    fontSize: "1.125rem",
    lineHeight: 1.25,
    fontWeight: 600,
    textWrap: "balance",
    color: "#181613",
  },
  style12: {
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#ede7dc",
    paddingTop: ".75rem",
    fontSize: ".75rem",
    lineHeight: "1.25rem",
    color: "#756b5d",
  },
  style13: {
    position: "relative",
    paddingTop: 0,
    paddingBottom: {
      default: ".5rem",
      "@media (width >= 48rem)": 0,
    },
  },
  style14: {
    clipPath: "inset(50%)",
    whiteSpace: "nowrap",
    borderWidth: 0,
    width: "1px",
    height: "1px",
    margin: "-1px",
    padding: 0,
    position: "absolute",
    overflow: "hidden",
  },
  style15: {
    pointerEvents: "none",
    position: "absolute",
    top: "-4.4rem",
    left: "50%",
    zIndex: 20,
    height: "5rem",
    width: "15rem",
    translate: "calc(160% * -1) 0",
    color: "#0a0a0a",
    display: {
      default: null,
      "@media (width < 899px)": "none",
    },
  },
  style16: {
    position: "absolute",
    top: 0,
    left: 0,
    width: "max-content",
    rotate: "-3deg",
    fontFamily: "Reenie Beanie, Patrick Hand, cursive",
    fontSize: {
      default: "25px",
      "@media (width >= 64rem)": "28px",
    },
    lineHeight: 1,
    fontWeight: 400,
    whiteSpace: "nowrap",
  },
  style17: {
    position: "absolute",
    top: "1.65rem",
    left: "1.15rem",
    height: "2.9rem",
    width: "4.65rem",
    rotate: "5deg",
    color: "#0a0a0a",
  },
  style18: {
    position: "relative",
    left: "50%",
    width: "100vw",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    overflow: {
      default: "hidden",
      "@media (prefers-reduced-motion: reduce)": "visible",
    },
    backgroundColor: "#fff",
  },
  style19: {
    display: "flex",
    width: {
      default: "max-content",
      "@media (prefers-reduced-motion: reduce)": "100%",
    },
    maxWidth: {
      default: null,
      [media.reducedMotion]: "72rem",
    },
    alignItems: "center",
    marginInline: {
      default: null,
      "@media (prefers-reduced-motion: reduce)": "auto",
    },
    animationDuration: "36s",
    animationIterationCount: "infinite",
    animationName: {
      default: scrollLeft,
      [media.reducedMotion]: "none",
    },
    animationTimingFunction: "linear",
    justifyContent: {
      default: null,
      "@media (prefers-reduced-motion: reduce)": "center",
    },
    paddingInline: {
      default: null,
      "@media (prefers-reduced-motion: reduce)": "1.5rem",
    },
  },
  style20: {
    pointerEvents: "none",
    position: "absolute",
    insetBlock: 0,
    left: 0,
    width: {
      default: "4rem",
      "@media (width >= 48rem)": "8rem",
    },
    backgroundImage: `linear-gradient(to right, ${colors.card}, transparent)`,
    display: {
      default: null,
      "@media (prefers-reduced-motion: reduce)": "none",
    },
  },
  style21: {
    pointerEvents: "none",
    position: "absolute",
    insetBlock: 0,
    right: 0,
    width: {
      default: "4rem",
      "@media (width >= 48rem)": "8rem",
    },
    backgroundImage: `linear-gradient(to left, ${colors.card}, transparent)`,
    display: {
      default: null,
      "@media (prefers-reduced-motion: reduce)": "none",
    },
  },
  style22: {
    marginTop: ".25rem",
    fontFamily: "Reenie Beanie, Patrick Hand, cursive",
    fontSize: "22px",
    lineHeight: 1,
    fontWeight: 400,
    color: "#0a0a0a",
    clipPath: {
      default: null,
      "@media (width >= 900px)": "inset(50%)",
    },
    whiteSpace: {
      default: null,
      "@media (width >= 900px)": "nowrap",
    },
    borderWidth: {
      default: null,
      "@media (width >= 900px)": 0,
    },
    width: {
      default: null,
      "@media (width >= 900px)": "1px",
    },
    height: {
      default: null,
      "@media (width >= 900px)": "1px",
    },
    margin: {
      default: null,
      "@media (width >= 900px)": "-1px",
    },
    padding: {
      default: null,
      "@media (width >= 900px)": 0,
    },
    position: {
      default: null,
      "@media (width >= 900px)": "absolute",
    },
    overflow: {
      default: null,
      "@media (width >= 900px)": "hidden",
    },
  },
  style23: {
    paddingBlock: {
      default: "4rem",
      "@media (width >= 48rem)": "5rem",
    },
  },
  style24: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#756b5d",
  },
  style25: {
    marginInline: "auto",
    marginTop: "1.5rem",
    maxWidth: "42rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    color: "#4f4940",
  },
  style26: {
    position: "relative",
    left: "50%",
    marginInline: "auto",
    marginTop: "2rem",
    height: {
      default: "19rem",
      "@media (width >= 40rem)": "18rem",
    },
    width: "100vw",
    maxWidth: "980px",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    overflow: "visible",
    paddingInline: "1.25rem",
  },
  style27: {
    position: "absolute",
    top: 0,
    left: "50%",
    zIndex: 0,
    display: "flex",
    height: {
      default: "15.5rem",
      "@media (width >= 40rem)": "13.5rem",
    },
    width: {
      default: "calc(100% - 2.5rem)",
      "@media (width >= 40rem)": "380px",
    },
    maxWidth: "380px",
    translate: {
      default: "calc(calc(1 / 2 * 100%) * -1) 0",
      "@media (width >= 40rem)": "0 34px",
    },
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    textAlign: "center",
  },
  style28: {
    fontFamily: fonts.hand,
    fontSize: {
      default: "1.875rem",
      "@media (width >= 40rem)": "2.25rem",
    },
    lineHeight: {
      default: 1,
      "@media (width >= 40rem)": "2.5rem",
    },
    fontWeight: 600,
    color: "#181613",
  },
  style29: {
    marginTop: "1.5rem",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  style30: {
    display: "inline-flex",
    alignItems: "center",
    gap: ".5rem",
    borderRadius: radii.full,
    backgroundColor: {
      default: "#181613",
      ":hover": "#4f4940",
    },
    paddingInline: "1.25rem",
    paddingBlock: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#fff",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style31: {
    display: {
      default: null,
      "@media (width >= 40rem)": "none",
    },
  },
  style32: {
    display: {
      default: "none",
      "@media (width >= 40rem)": "block",
    },
  },
  testimonialCard: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    cornerShape: "squircle",
    padding: "1.25rem",
    position: "absolute",
    textAlign: "left",
    transitionDuration: {
      default: "500ms",
      [media.reducedMotion]: "0ms",
    },
    transitionProperty: "transform, box-shadow, opacity",
    transitionTimingFunction: "cubic-bezier(0.22, 1, 0.36, 1)",
    userSelect: "none",
  },
  logoTrack: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: {
      default: "3.5rem",
      [media.md]: "5rem",
    },
    paddingInline: {
      default: "1.75rem",
      [media.md]: "2.5rem",
    },
  },
  logoTrackReducedPrimary: {
    columnGap: {
      default: "3rem",
      [media.md]: "4rem",
    },
    flexShrink: {
      default: 0,
      [media.reducedMotion]: 1,
    },
    flexWrap: {
      default: "nowrap",
      [media.reducedMotion]: "wrap",
    },
    justifyContent: {
      default: "flex-start",
      [media.reducedMotion]: "center",
    },
    paddingInline: {
      default: null,
      [media.reducedMotion]: 0,
    },
    rowGap: {
      default: null,
      [media.reducedMotion]: "1.5rem",
    },
    width: {
      default: null,
      [media.reducedMotion]: "100%",
    },
  },
  logoTrackReducedDuplicate: {
    display: {
      default: "flex",
      [media.reducedMotion]: "none",
    },
  },
  credibilityLogo: {
    filter: {
      default: "grayscale(100%)",
      ":hover": "grayscale(0)",
    },
    height: "1.5rem",
    maxWidth: "none",
    objectFit: "contain",
    opacity: {
      default: 0.65,
      ":hover": 1,
    },
    transitionDuration: "200ms",
    transitionProperty: "filter, opacity",
    width: "auto",
  },
  compactCredibilityLogo: {
    maxHeight: "1.25rem",
  },
  mobileTestimonialCard: {
    boxShadow: {
      default: "0 24px 60px rgb(24 22 19 / 0.14)",
      ":hover": "0 30px 75px rgb(24 22 19 / 0.16)",
    },
    cursor: "pointer",
    height: "15.5rem",
    left: "50%",
    top: 0,
    width: "calc(100% - 2.5rem)",
  },
  testimonialCardDisabled: {
    pointerEvents: "none",
  },
  desktopTestimonialCard: {
    cursor: "pointer",
    height: "13.5rem",
    left: "50%",
    top: 0,
    width: "380px",
  },
  desktopTestimonialCardTop: {
    boxShadow: {
      default: "0 24px 60px rgb(24 22 19 / 0.14)",
      ":hover": "0 30px 75px rgb(24 22 19 / 0.16)",
    },
  },
  desktopTestimonialCardStacked: {
    boxShadow: {
      default: "0 14px 36px rgb(24 22 19 / 0.1)",
      ":hover": "0 18px 44px rgb(24 22 19 / 0.13)",
    },
  },
});
const credibilityLogos: {
  name: string;
  src: string;
  width: number;
  height: number;
  compact?: boolean;
  resizeWidth?: number;
}[] = [
  {
    name: "Databricks",
    src: "/icons/databricks.svg",
    width: 276,
    height: 42,
    compact: true,
  },
  {
    name: "Cloudflare",
    src: "/icons/cloudflare.png",
    width: 1556,
    height: 704,
    resizeWidth: 106,
  },
  {
    name: "Amazon",
    src: "/icons/amazon.svg",
    width: 399,
    height: 133,
    compact: true,
  },
  {
    name: "Meta",
    src: "/icons/meta.svg",
    width: 256,
    height: 171,
    compact: true,
  },
  {
    name: "Y Combinator",
    src: "/icons/yc.svg",
    width: 64,
    height: 64,
  },
  {
    name: "Palantir",
    src: "/icons/palantir.svg",
    width: 210,
    height: 51,
    compact: true,
  },
  {
    name: "Apple",
    src: "/icons/apple.svg",
    width: 42,
    height: 51,
    compact: true,
  },
  {
    name: "Disney",
    src: "/icons/disney.svg",
    width: 155,
    height: 66,
    compact: true,
  },
  {
    name: "Richmond American",
    src: "/icons/richmond_american.svg",
    width: 165,
    height: 51,
    compact: true,
  },
  {
    name: "Adobe",
    src: "/icons/adobe.svg",
    width: 66,
    height: 17,
    compact: true,
  },
  {
    name: "Wayfair",
    src: "/icons/wayfair.svg",
    width: 630,
    height: 150,
    compact: true,
  },
  {
    name: "Bain & Company",
    src: "/icons/bain.svg",
    width: 547,
    height: 60,
    compact: true,
  },
  {
    name: "McKinsey & Company",
    src: "/icons/mckinsey.png",
    width: 960,
    height: 297,
    compact: true,
    resizeWidth: 130,
  },
];
const testimonials = [
  {
    quote: "Anarlog is great and local.",
    author: "Tobi Lutke",
    username: "tobi",
    avatar: "/images/blog/testimonials/tobi.jpg",
    url: "https://x.com/tobi/status/1983892259230699921",
  },
  {
    quote: "Anarlog is worth a look.",
    author: "Anand Chowdhary",
    username: "AnandChowdhary",
    avatar: "/images/blog/testimonials/anand.jpg",
    url: "https://x.com/AnandChowdhary/status/1997980479698723119",
  },
  {
    quote: "Anarlog is one of my favorite AI secret weapons.",
    author: "James Koshigoe",
    username: "JamesKoshigoe",
    avatar: "/images/blog/testimonials/james-k.jpg",
    url: "https://x.com/JamesKoshigoe/status/2024676687980671195",
  },
  {
    quote: "Really liking Anarlog. Open access to my data and a GPL codebase!",
    author: "James LePage",
    username: "jameswlepage",
    avatar: "/images/blog/testimonials/james-l.jpg",
    url: "https://x.com/jameswlepage/status/2042780872693166169",
  },
  {
    quote:
      "I love the flexibility that Anarlog gives me to integrate personal notes with AI summaries.",
    author: "Tom Yang",
    username: "tomyang11_",
    avatar: "/images/blog/testimonials/tom.jpg",
    url: "https://twitter.com/tomyang11_/status/1956395933538902092",
  },
];
type TestimonialCardPosition = {
  x: number | string;
  y: number;
  rotate: number;
  scale: number;
};
const mobileTestimonialPilePositions: TestimonialCardPosition[] = [
  {
    x: 0,
    y: 0,
    rotate: -0.5,
    scale: 1,
  },
  {
    x: 7,
    y: 12,
    rotate: 1.1,
    scale: 0.985,
  },
  {
    x: -7,
    y: 24,
    rotate: -1.4,
    scale: 0.97,
  },
  {
    x: 9,
    y: 36,
    rotate: 1.7,
    scale: 0.955,
  },
  {
    x: -9,
    y: 48,
    rotate: -1.7,
    scale: 0.94,
  },
];
const mobileTestimonialSidePositions: TestimonialCardPosition[] = [
  {
    x: "calc(5.75rem - 100vw)",
    y: 0,
    rotate: -6,
    scale: 0.94,
  },
  {
    x: "calc(100vw - 5.75rem)",
    y: 16,
    rotate: 6,
    scale: 0.94,
  },
  {
    x: "calc(5.25rem - 100vw)",
    y: 44,
    rotate: 5,
    scale: 0.9,
  },
  {
    x: "calc(100vw - 5.25rem)",
    y: 60,
    rotate: -5,
    scale: 0.9,
  },
  {
    x: "calc(5.75rem - 100vw)",
    y: 80,
    rotate: -2.5,
    scale: 0.86,
  },
];
const desktopTestimonialPilePositions: TestimonialCardPosition[] = [
  {
    x: 0,
    y: 34,
    rotate: -1.5,
    scale: 1,
  },
  {
    x: 10,
    y: 44,
    rotate: 1.7,
    scale: 0.985,
  },
  {
    x: -11,
    y: 54,
    rotate: -2.2,
    scale: 0.97,
  },
  {
    x: 14,
    y: 64,
    rotate: 2.8,
    scale: 0.955,
  },
  {
    x: -14,
    y: 74,
    rotate: -3,
    scale: 0.94,
  },
];
const desktopTestimonialSidePositions: TestimonialCardPosition[] = [
  {
    x: -430,
    y: 0,
    rotate: -7,
    scale: 0.9,
  },
  {
    x: 430,
    y: 8,
    rotate: 7,
    scale: 0.9,
  },
  {
    x: -420,
    y: 132,
    rotate: 6,
    scale: 0.88,
  },
  {
    x: 420,
    y: 140,
    rotate: -6,
    scale: 0.88,
  },
  {
    x: -430,
    y: 74,
    rotate: -3,
    scale: 0.82,
  },
];
const testimonialDeckStateVersion = 3;
const testimonialNameContext =
  "Name context: Hyprnote became Char, then Anarlog.";
function formatTestimonialOffset(offset: TestimonialCardPosition["x"]) {
  return typeof offset === "number" ? `${offset}px` : offset;
}
function renderPullQuote(quote: string) {
  return quote.split(/(Anarlog)/g).map((part, index) => {
    if (part !== "Anarlog") return part;
    return (
      <mark key={index} {...stylex.props(styles.style1)}>
        {part}
      </mark>
    );
  });
}
function TestimonialTweetCard({
  testimonial,
  ariaLabel,
  sx,
  style,
  onMoveToSide,
}: {
  testimonial: (typeof testimonials)[number];
  ariaLabel: string;
  style?: CSSProperties;
  onMoveToSide: () => void;
} & StyleXProps) {
  return (
    <article
      role="button"
      tabIndex={0}
      aria-label={ariaLabel}
      onClick={onMoveToSide}
      onKeyDown={(event) => {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        onMoveToSide();
      }}
      {...mergeStyleXProps([styles.testimonialCard, sx], undefined, style)}
    >
      <figure {...stylex.props(styles.style2)}>
        <figcaption {...stylex.props(styles.style3)}>
          <div {...stylex.props(styles.style4)}>
            <img
              src={getResizedImageUrl(testimonial.avatar, {
                width: 48,
                height: 48,
              })}
              srcSet={getResizedImageSrcSet(testimonial.avatar, 48)}
              alt={`${testimonial.author} profile photo`}
              width={48}
              height={48}
              {...stylex.props(styles.style5)}
              decoding="async"
              loading="lazy"
            />
            <div {...stylex.props(styles.style6)}>
              <p {...stylex.props(styles.style7)}>{testimonial.author}</p>
              <p {...stylex.props(styles.style8)}>@{testimonial.username}</p>
            </div>
          </div>

          <a
            href={testimonial.url}
            target="_blank"
            rel="noreferrer"
            aria-label={`View ${testimonial.author} post on X`}
            onClick={(event) => event.stopPropagation()}
            {...stylex.props(styles.style9)}
          >
            <XLogo size={15} aria-hidden="true" />
          </a>
        </figcaption>

        <blockquote {...stylex.props(styles.style10)}>
          <p {...stylex.props(styles.style11)}>
            {renderPullQuote(testimonial.quote)}
          </p>
        </blockquote>

        <p {...stylex.props(styles.style12)}>{testimonialNameContext}</p>
      </figure>
    </article>
  );
}
export function CredibilityLogoMarquee() {
  return (
    <section
      {...stylex.props(styles.style13)}
      aria-labelledby="credibility-heading"
    >
      <p {...stylex.props(styles.style14)}>
        {credibilityLogos.map((logo) => logo.name).join(", ")}
      </p>
      <div {...stylex.props(styles.style15)} aria-hidden="true">
        <p {...stylex.props(styles.style16)}>people love us at</p>
        <svg {...stylex.props(styles.style17)} viewBox="0 0 74 46" fill="none">
          <path
            d="M7 8L56 30"
            stroke="currentColor"
            strokeWidth="2.1"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M44 22L57 30L42 37"
            stroke="currentColor"
            strokeWidth="2.1"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </div>

      <div {...stylex.props(styles.style18)}>
        <div {...stylex.props(styles.style19)} aria-hidden="true">
          {[0, 1].map((trackIndex) => (
            <div
              key={trackIndex}
              {...stylex.props([
                styles.logoTrack,
                trackIndex === 0 && styles.logoTrackReducedPrimary,
                trackIndex === 1 && styles.logoTrackReducedDuplicate,
              ])}
            >
              {credibilityLogos.map((logo) => (
                <img
                  key={`${trackIndex}-${logo.name}`}
                  src={
                    logo.resizeWidth
                      ? getResizedImageUrl(logo.src, {
                          width: logo.resizeWidth,
                        })
                      : logo.src
                  }
                  width={logo.width}
                  height={logo.height}
                  alt=""
                  {...stylex.props([
                    styles.credibilityLogo,
                    logo.compact && styles.compactCredibilityLogo,
                  ])}
                  draggable={false}
                />
              ))}
            </div>
          ))}
        </div>
        <div {...stylex.props(styles.style20)} aria-hidden="true" />
        <div {...stylex.props(styles.style21)} aria-hidden="true" />
      </div>
      <h2 id="credibility-heading" {...stylex.props(styles.style22)}>
        people love us at
      </h2>
    </section>
  );
}
export function TestimonialsSection() {
  const [testimonialDeckState, setTestimonialDeckState] = useState({
    version: testimonialDeckStateVersion,
    movedIndexes: [] as number[],
  });
  const movedTestimonialIndexes =
    testimonialDeckState.version === testimonialDeckStateVersion
      ? testimonialDeckState.movedIndexes
      : [];
  const movedTestimonialSet = new Set(movedTestimonialIndexes);
  const remainingTestimonialIndexes = testimonials
    .map((_, index) => index)
    .filter((index) => !movedTestimonialSet.has(index));
  const handleMoveToSide = (itemIndex: number) => {
    setTestimonialDeckState((currentState) => {
      const currentIndexes =
        currentState.version === testimonialDeckStateVersion
          ? currentState.movedIndexes
          : [];
      if (currentIndexes.includes(itemIndex)) return currentState;
      return {
        version: testimonialDeckStateVersion,
        movedIndexes: [...currentIndexes, itemIndex],
      };
    });
  };
  return (
    <section {...stylex.props(styles.style23)}>
      <div>
        <h2 {...stylex.props(styles.style24)}>What people say</h2>
        <p {...stylex.props(styles.style25)}>It's clear they love Anarlog.</p>
      </div>

      <div {...stylex.props(styles.style26)}>
        <div {...stylex.props(styles.style27)}>
          <p {...stylex.props(styles.style28)}>Try for yourself.</p>
          <div {...stylex.props(styles.style29)}>
            <Link to="/download/" {...stylex.props(styles.style30)}>
              Start using for free
              <ArrowRight size={16} weight="bold" aria-hidden="true" />
            </Link>
          </div>
        </div>

        <div {...stylex.props(styles.style31)}>
          {testimonials.map((testimonial, itemIndex) => {
            const movedIndex = movedTestimonialIndexes.indexOf(itemIndex);
            const isMoved = movedIndex >= 0;
            const remainingIndex =
              remainingTestimonialIndexes.indexOf(itemIndex);
            const pilePosition = isMoved
              ? mobileTestimonialSidePositions[
                  movedIndex % mobileTestimonialSidePositions.length
                ]
              : mobileTestimonialPilePositions[
                  Math.max(remainingIndex, 0) %
                    mobileTestimonialPilePositions.length
                ];
            return (
              <TestimonialTweetCard
                key={itemIndex}
                testimonial={testimonial}
                ariaLabel={`Move ${testimonial.author} testimonial to the side`}
                onMoveToSide={() => handleMoveToSide(itemIndex)}
                sx={[
                  styles.mobileTestimonialCard,
                  (isMoved || remainingIndex > 0) &&
                    styles.testimonialCardDisabled,
                ]}
                style={{
                  transform: `translate(calc(-50% + ${formatTestimonialOffset(pilePosition.x)}), ${pilePosition.y}px) scale(${pilePosition.scale}) rotate(${pilePosition.rotate}deg)`,
                  transformOrigin: "top center",
                  zIndex: isMoved
                    ? 20 + movedIndex
                    : 40 + remainingTestimonialIndexes.length - remainingIndex,
                }}
              />
            );
          })}
        </div>

        <div {...stylex.props(styles.style32)}>
          {testimonials.map((testimonial, itemIndex) => {
            const movedIndex = movedTestimonialIndexes.indexOf(itemIndex);
            const isMoved = movedIndex >= 0;
            const remainingIndex =
              remainingTestimonialIndexes.indexOf(itemIndex);
            const pilePosition = isMoved
              ? desktopTestimonialSidePositions[
                  movedIndex % desktopTestimonialSidePositions.length
                ]
              : desktopTestimonialPilePositions[
                  Math.max(remainingIndex, 0) %
                    desktopTestimonialPilePositions.length
                ];
            return (
              <TestimonialTweetCard
                key={itemIndex}
                testimonial={testimonial}
                ariaLabel={`Move ${testimonial.author} testimonial to the side`}
                onMoveToSide={() => handleMoveToSide(itemIndex)}
                sx={[
                  styles.desktopTestimonialCard,
                  !isMoved && remainingIndex === 0
                    ? styles.desktopTestimonialCardTop
                    : styles.desktopTestimonialCardStacked,
                ]}
                style={{
                  transform: `translate(calc(-50% + ${formatTestimonialOffset(pilePosition.x)}), ${pilePosition.y}px) scale(${pilePosition.scale}) rotate(${pilePosition.rotate}deg)`,
                  transformOrigin: "top center",
                  zIndex: isMoved
                    ? 20 + movedIndex
                    : 40 + remainingTestimonialIndexes.length - remainingIndex,
                }}
              />
            );
          })}
        </div>
      </div>
    </section>
  );
}
