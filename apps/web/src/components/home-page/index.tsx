import { Icon } from "@iconify-icon/react";
import { ArrowRight } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";

import { radii, fonts } from "@anlg/design-system/tokens.stylex";

import { SiteFooter } from "@/components/site-footer";
import { getResizedImageSrcSet, getResizedImageUrl } from "@/lib/image-cdn";
import { MANIFESTO_SIGNERS } from "@/lib/team";

import { HeroSection } from "./hero-section";
import { PricingSection } from "./pricing-section";
import { PrivacySection } from "./privacy-section";
import { TestimonialsSection } from "./social-proof-sections";
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
      default: "7rem",
      "@media (width >= 48rem)": "8rem",
    },
    paddingBottom: {
      default: "3.5rem",
      "@media (width >= 48rem)": "4rem",
    },
  },
  style5: {
    marginInline: "auto",
    maxWidth: "48rem",
    overflow: "hidden",
    borderRadius: "3px",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#eadfce",
    backgroundColor: "#fffaf0",
    backgroundImage:
      "linear-gradient(115deg, rgba(255, 250, 240, 0.9), rgba(246, 236, 218, 0.82)), url('/textures/crumpled-paper.webp')",
    backgroundPosition: "center",
    backgroundSize: "cover",
    paddingInline: {
      default: "1.75rem",
      "@media (width >= 40rem)": "2.5rem",
    },
    paddingBlock: {
      default: "2.25rem",
      "@media (width >= 40rem)": "3rem",
    },
    textAlign: "left",
    boxShadow: "0 18px 50px #4436241f",
  },
  style6: {
    display: "flex",
    flexDirection: "column",
    gap: "1.5rem",
    color: "#363029",
  },
  style7: {
    fontSize: "18px",
    lineHeight: "2rem",
  },
  style8: {
    marginTop: "2.5rem",
    display: "flex",
    width: "100%",
    flexDirection: "column",
    alignItems: "flex-start",
    paddingTop: ".5rem",
  },
  style9: {
    display: "flex",
    width: "fit-content",
    maxWidth: "100%",
    flexDirection: "column",
    alignItems: "flex-start",
    gap: ".75rem",
  },
  style10: {
    display: "flex",
    gap: ".25rem",
  },
  style11: {
    display: "block",
    width: "30px",
    height: "30px",
    flexShrink: 0,
    overflow: "hidden",
    borderRadius: radii.full,
    transitionProperty: "transform, translate, scale, rotate",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    translate: {
      default: null,
      ":hover": "0 calc(.125rem * -1)",
    },
    outlineStyle: {
      default: null,
      ":focus-visible": "solid",
    },
    outlineWidth: {
      default: null,
      ":focus-visible": "2px",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    outlineColor: {
      default: null,
      ":focus-visible": "#181613",
    },
  },
  style12: {
    width: "100%",
    height: "100%",
    objectFit: "cover",
  },
  style13: {
    display: "block",
    width: "30px",
    height: "30px",
    flexShrink: 0,
    overflow: "hidden",
    borderRadius: radii.full,
  },
  style14: {
    fontSize: "12px",
    lineHeight: 1,
    letterSpacing: ".04em",
    color: "#756b5d",
  },
  style15: {
    position: "relative",
    left: "50%",
    marginTop: {
      default: "2.5rem",
      "@media (width >= 48rem)": "3rem",
    },
    width: "100vw",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    paddingBlock: {
      default: "5rem",
      "@media (width >= 48rem)": "6rem",
    },
  },
  style16: {
    marginInline: "auto",
    maxWidth: "700px",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    textAlign: "center",
  },
  style17: {
    marginInline: "auto",
    maxWidth: "48rem",
    fontFamily: fonts.hand,
    fontSize: {
      default: "2.25rem",
      "@media (width >= 48rem)": "3rem",
    },
    lineHeight: {
      default: 0.98,
      "@media (width >= 48rem)": 1,
    },
    fontWeight: 600,
    letterSpacing: 0,
    textWrap: "balance",
    color: "#181613",
  },
  style18: {
    marginInline: "auto",
    marginTop: "1.5rem",
    maxWidth: "42rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    color: "#4f4940",
  },
  style19: {
    marginTop: "2rem",
    display: "inline-flex",
    alignItems: "center",
    gap: ".5rem",
    borderRadius: radii.full,
    backgroundColor: "#181613",
    paddingInline: "1.25rem",
    paddingBlock: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#fff",
  },
  style20: {
    position: "relative",
    left: "50%",
    width: "100vw",
    maxWidth: "880px",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    paddingBlock: {
      default: "3rem",
      "@media (width >= 48rem)": "3.5rem",
    },
  },
  style21: {
    marginInline: "auto",
    maxWidth: "700px",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
  },
  style22: {
    fontFamily: fonts.hand,
    fontSize: "1.875rem",
    lineHeight: 1,
    fontWeight: 600,
    color: "#756b5d",
  },
  style23: {
    marginInline: "auto",
    marginTop: "1.25rem",
    maxWidth: "42rem",
    fontSize: "1.125rem",
    lineHeight: "2rem",
    color: "#4f4940",
  },
  style24: {
    marginTop: "1.5rem",
    display: "inline-flex",
    alignItems: "center",
    gap: ".5rem",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: {
      default: "#d4d4d4",
      ":hover": "#171717",
    },
    backgroundColor: {
      default: "#fff",
      ":hover": "#171717",
    },
    paddingInline: "1.25rem",
    paddingBlock: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: {
      default: "#171717",
      ":hover": "#fff",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style25: {
    flexShrink: 0,
  },
});
const manifestoLetter = [
  "To the people who still take notes,",
  "Notetaking matters more than note-takers. A note-taker is passive. A notepad is something you use. You stay present and in control while the room is still alive.",
  "Most AI tools ask you to move your memory into their ecosystem and rules. Meeting notes should stay in software you control, with a local record that can work offline.",
  "Interfaces change. Your meeting record should remain yours. Use on-device models or your own keys, not a service you cannot inspect.",
  "Anarlog is our attempt to build that meeting notepad.",
];
const manifestoSigners = MANIFESTO_SIGNERS;
export function HomePage({
  formattedGithubStars,
}: {
  formattedGithubStars: string;
}) {
  return (
    <main {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <div {...stylex.props(styles.style3)}>
          <HeroSection />

          <PrivacySection />

          <TestimonialsSection />

          <OpenSourceSection formattedGithubStars={formattedGithubStars} />

          <PricingSection compareLink />

          <section id="manifesto" {...stylex.props(styles.style4)}>
            <article {...stylex.props(styles.style5)}>
              <div {...stylex.props(styles.style6)}>
                {manifestoLetter.map((paragraph) => (
                  <p key={paragraph} {...stylex.props(styles.style7)}>
                    {paragraph}
                  </p>
                ))}
              </div>
              <div {...stylex.props(styles.style8)}>
                <div {...stylex.props(styles.style9)}>
                  <div {...stylex.props(styles.style10)}>
                    {manifestoSigners.map((member) =>
                      member.links.twitter ? (
                        <a
                          key={member.id}
                          href={member.links.twitter}
                          target="_blank"
                          rel="noopener noreferrer"
                          aria-label={`${member.name} on X`}
                          {...stylex.props(styles.style11)}
                        >
                          <img
                            src={getResizedImageUrl(member.avatar, {
                              width: 30,
                              height: 30,
                            })}
                            srcSet={getResizedImageSrcSet(member.avatar, 30)}
                            alt=""
                            width={30}
                            height={30}
                            {...stylex.props(styles.style12)}
                            decoding="async"
                            loading="lazy"
                          />
                        </a>
                      ) : (
                        <span
                          key={member.id}
                          aria-label={`${member.name} profile picture`}
                          {...stylex.props(styles.style13)}
                          role="img"
                        >
                          <img
                            src={getResizedImageUrl(member.avatar, {
                              width: 30,
                              height: 30,
                            })}
                            srcSet={getResizedImageSrcSet(member.avatar, 30)}
                            alt=""
                            width={30}
                            height={30}
                            {...stylex.props(styles.style12)}
                            decoding="async"
                            loading="lazy"
                          />
                        </span>
                      ),
                    )}
                  </div>
                  <p {...stylex.props(styles.style14)}>
                    {manifestoSigners
                      .map((member) => member.name.split(" ")[0])
                      .join(", ")}
                  </p>
                </div>
              </div>
            </article>
          </section>

          <FinalCtaSection />
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}
function FinalCtaSection() {
  return (
    <section {...stylex.props(styles.style15)}>
      <div {...stylex.props(styles.style16)}>
        <h2 {...stylex.props(styles.style17)}>
          Keep your meeting notes yours.
        </h2>
        <p {...stylex.props(styles.style18)}>
          Try Anarlog today and be present in meetings.
        </p>
        <Link to="/download/" {...stylex.props(styles.style19)}>
          <span>Download for free</span>
          <ArrowRight size={16} weight="bold" aria-hidden="true" />
        </Link>
      </div>
    </section>
  );
}
function OpenSourceSection({
  formattedGithubStars,
}: {
  formattedGithubStars: string;
}) {
  return (
    <section
      {...stylex.props(styles.style20)}
      aria-labelledby="open-source-heading"
    >
      <div {...stylex.props(styles.style21)}>
        <h2 id="open-source-heading" {...stylex.props(styles.style22)}>
          Open source by default
        </h2>
        <p {...stylex.props(styles.style23)}>
          We deeply care about transparency. Anarlog is open source so anyone
          can inspect how meeting memory is handled.
        </p>

        <a
          href="https://github.com/fastrepl/anarlog"
          target="_blank"
          rel="noopener noreferrer"
          {...stylex.props(styles.style24)}
        >
          <Icon
            icon="simple-icons:github"
            width={18}
            height={18}
            {...stylex.props(styles.style25)}
            aria-hidden="true"
          />
          <span>{formattedGithubStars} stars on GitHub</span>
        </a>
      </div>
    </section>
  );
}
