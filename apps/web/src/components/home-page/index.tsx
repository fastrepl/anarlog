import { Icon } from "@iconify-icon/react";
import { ArrowRight } from "@phosphor-icons/react";
import { Link } from "@tanstack/react-router";

import { SiteFooter } from "@/components/site-footer";
import { getResizedImageSrcSet, getResizedImageUrl } from "@/lib/image-cdn";
import { MANIFESTO_SIGNERS } from "@/lib/team";

import { HeroSection } from "./hero-section";
import { PricingSection } from "./pricing-section";
import { PrivacySection } from "./privacy-section";
import { TestimonialsSection } from "./social-proof-sections";

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
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 pt-4 pb-8 md:px-8 md:pt-4 md:pb-12">
        <div className="min-w-0 text-center">
          <HeroSection />

          <PrivacySection />

          <TestimonialsSection />

          <OpenSourceSection formattedGithubStars={formattedGithubStars} />

          <PricingSection />

          <section id="manifesto" className="pt-28 pb-14 md:pt-32 md:pb-16">
            <article
              className="mx-auto max-w-3xl overflow-hidden rounded-[3px] border border-[#eadfce] bg-[#fffaf0] px-7 py-9 text-left shadow-[0_18px_50px_rgba(68,54,36,0.12)] sm:px-10 sm:py-12"
              style={{
                backgroundImage:
                  "linear-gradient(115deg, rgba(255, 250, 240, 0.9), rgba(246, 236, 218, 0.82)), url('/textures/crumpled-paper.webp')",
                backgroundPosition: "center",
                backgroundSize: "cover",
              }}
            >
              <div className="space-y-6 text-[#363029]">
                {manifestoLetter.map((paragraph) => (
                  <p key={paragraph} className="text-[18px] leading-8">
                    {paragraph}
                  </p>
                ))}
              </div>
              <div className="mt-10 flex w-full flex-col items-start pt-2">
                <div className="flex w-fit max-w-full flex-col items-start gap-3">
                  <div className="flex gap-1">
                    {manifestoSigners.map((member) =>
                      member.links.twitter ? (
                        <a
                          key={member.id}
                          href={member.links.twitter}
                          target="_blank"
                          rel="noopener noreferrer"
                          aria-label={`${member.name} on X`}
                          className="block size-[30px] shrink-0 overflow-hidden rounded-full transition-transform hover:-translate-y-0.5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#181613]"
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
                            className="size-full object-cover"
                            decoding="async"
                            loading="lazy"
                          />
                        </a>
                      ) : (
                        <span
                          key={member.id}
                          aria-label={`${member.name} profile picture`}
                          className="block size-[30px] shrink-0 overflow-hidden rounded-full"
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
                            className="size-full object-cover"
                            decoding="async"
                            loading="lazy"
                          />
                        </span>
                      ),
                    )}
                  </div>
                  <p className="text-[12px] leading-none tracking-[0.04em] text-[#756b5d]">
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
    <section className="relative left-1/2 mt-10 w-screen -translate-x-1/2 py-20 md:mt-12 md:py-24">
      <div className="mx-auto max-w-[700px] px-5 text-center md:px-8">
        <h2 className="font-hand mx-auto max-w-3xl text-4xl leading-[0.98] font-semibold tracking-normal text-balance text-[#181613] md:text-5xl">
          Keep your meeting notes yours.
        </h2>
        <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
          Try Anarlog today and be present in meetings.
        </p>
        <Link
          to="/download/"
          className="mt-8 inline-flex items-center gap-2 rounded-full bg-[#181613] px-5 py-3 text-sm font-medium text-white"
        >
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
      className="relative left-1/2 w-screen max-w-[880px] -translate-x-1/2 py-12 md:py-14"
      aria-labelledby="open-source-heading"
    >
      <div className="mx-auto max-w-[700px] px-5 md:px-8">
        <h2
          id="open-source-heading"
          className="font-hand text-3xl leading-none font-semibold text-[#756b5d]"
        >
          Open source by default
        </h2>
        <p className="mx-auto mt-5 max-w-2xl text-lg leading-8 text-[#4f4940]">
          We deeply care about transparency. Anarlog is open source so anyone
          can inspect how meeting memory is handled.
        </p>

        <a
          href="https://github.com/fastrepl/anarlog"
          target="_blank"
          rel="noopener noreferrer"
          className="mt-6 inline-flex items-center gap-2 rounded-full border border-neutral-300 bg-white px-5 py-3 text-sm font-medium text-neutral-900 transition-colors hover:border-neutral-900 hover:bg-neutral-900 hover:text-white"
        >
          <Icon icon="mdi:github" width={18} height={18} aria-hidden="true" />
          <span>{formattedGithubStars} stars on GitHub</span>
        </a>
      </div>
    </section>
  );
}
