import { Icon } from "@iconify-icon/react";
import { createFileRoute, redirect } from "@tanstack/react-router";
import {
  ArrowRight,
  ChevronDown,
  Cloud,
  Cpu,
  KeyRound,
  type LucideIcon,
} from "lucide-react";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import { z } from "zod";

import {
  MARKETING_PLAN_TIERS,
  PRO_TRIAL_DAYS,
  type MarketingPlanData,
  PlanFeatureList,
} from "@hypr/pricing";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import { SiteFooter } from "@/components/site-footer";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getGitHubStats, getStargazers } from "@/functions/github";
import { useMountEffect } from "@/hooks/useMountEffect";
import { appleIntelDownloadUrl, appleSiliconDownloadUrl } from "@/lib/download";
import {
  ANARLOG_SITE_URL,
  ROOT_DESCRIPTION,
  getOrganizationJsonLd,
  getSoftwareApplicationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";
import { MANIFESTO_SIGNERS } from "@/lib/team";

const manifestoLetter = [
  "To the people who still take notes,",
  "Notetaking matters more than note-takers. A note-taker is passive. A notepad is something you use. You stay present and in control while the room is still alive.",
  "Most AI tools ask you to move your memory into their ecosystem and rules. Meeting notes should move the other way: back to files on your disk and software you can run offline.",
  "Files endure. Interfaces change. Your notes should survive us. Use on-device models or your own keys, not a service you cannot inspect.",
  "Anarlog is our attempt to build that meeting notepad.",
];

const manifestoSigners = MANIFESTO_SIGNERS;

const featureList = [
  "Bot-free meeting capture",
  "Fully offline notes",
  "On-device or bring-your-own-key AI",
  "File-based storage",
  "Open source foundations",
];

const privacyCommitments = [
  {
    description: "Audio, transcripts, and notes stay as files on your device.",
    visual: "files",
  },
  {
    description: "Choose on-device models, your own key, or hosted AI.",
    visual: "key",
  },
  {
    description: "Capture meeting audio without adding a bot to the call.",
    visual: "meeting",
  },
];

const credibilityLogos = [
  { name: "Databricks", src: "/icons/databricks.svg", className: "max-h-5" },
  { name: "Cloudflare", src: "/icons/cloudflare.png" },
  { name: "Amazon", src: "/icons/amazon.svg", className: "max-h-5" },
  { name: "Meta", src: "/icons/meta.svg", className: "max-h-5" },
  { name: "Y Combinator", src: "/icons/yc.svg" },
  { name: "Palantir", src: "/icons/palantir.svg", className: "max-h-5" },
  { name: "Apple", src: "/icons/apple.svg", className: "max-h-5" },
  { name: "Disney", src: "/icons/disney.svg", className: "max-h-5" },
  {
    name: "Richmond American",
    src: "/icons/richmond_american.svg",
    className: "max-h-5",
  },
  { name: "Adobe", src: "/icons/adobe.svg", className: "max-h-5" },
  { name: "Wayfair", src: "/icons/wayfair.svg", className: "max-h-5" },
  { name: "Bain & Company", src: "/icons/bain.svg", className: "max-h-5" },
  {
    name: "McKinsey & Company",
    src: "/icons/mckinsey.png",
    className: "max-h-5",
  },
];

const testimonials = [
  {
    quote: "Anarlog is great and local.",
    author: "Tobi Lutke",
    username: "tobi",
    avatar: "/api/assets/blog/testimonials/tobi.jpg/",
    url: "https://x.com/tobi/status/1983892259230699921",
  },
  {
    quote: "Anarlog is worth a look.",
    author: "Anand Chowdhary",
    username: "AnandChowdhary",
    avatar: "/api/assets/blog/testimonials/anand.jpg/",
    url: "https://x.com/AnandChowdhary/status/1997980479698723119",
  },
  {
    quote: "Anarlog is one of my favorite AI secret weapons.",
    author: "James Koshigoe",
    username: "JamesKoshigoe",
    avatar: "/api/assets/blog/testimonials/james-k.jpg/",
    url: "https://x.com/JamesKoshigoe/status/2024676687980671195",
  },
  {
    quote: "Really liking Anarlog. Open access to my data and a GPL codebase!",
    author: "James LePage",
    username: "jameswlepage",
    avatar: "/api/assets/blog/testimonials/james-l.jpg/",
    url: "https://x.com/jameswlepage/status/2042780872693166169",
  },
  {
    quote:
      "I love the flexibility that Anarlog gives me to integrate personal notes with AI summaries.",
    author: "Tom Yang",
    username: "tomyang11_",
    avatar: "/api/assets/blog/testimonials/tom.jpg/",
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
  { x: 0, y: 0, rotate: -0.5, scale: 1 },
  { x: 7, y: 12, rotate: 1.1, scale: 0.985 },
  { x: -7, y: 24, rotate: -1.4, scale: 0.97 },
  { x: 9, y: 36, rotate: 1.7, scale: 0.955 },
  { x: -9, y: 48, rotate: -1.7, scale: 0.94 },
];

const mobileTestimonialSidePositions: TestimonialCardPosition[] = [
  { x: "calc(5.75rem - 100vw)", y: 0, rotate: -6, scale: 0.94 },
  { x: "calc(100vw - 5.75rem)", y: 16, rotate: 6, scale: 0.94 },
  { x: "calc(5.25rem - 100vw)", y: 44, rotate: 5, scale: 0.9 },
  { x: "calc(100vw - 5.25rem)", y: 60, rotate: -5, scale: 0.9 },
  { x: "calc(5.75rem - 100vw)", y: 80, rotate: -2.5, scale: 0.86 },
];

const desktopTestimonialPilePositions: TestimonialCardPosition[] = [
  { x: 0, y: 34, rotate: -1.5, scale: 1 },
  { x: 10, y: 44, rotate: 1.7, scale: 0.985 },
  { x: -11, y: 54, rotate: -2.2, scale: 0.97 },
  { x: 14, y: 64, rotate: 2.8, scale: 0.955 },
  { x: -14, y: 74, rotate: -3, scale: 0.94 },
];

const desktopTestimonialSidePositions: TestimonialCardPosition[] = [
  { x: -430, y: 0, rotate: -7, scale: 0.9 },
  { x: 430, y: 8, rotate: 7, scale: 0.9 },
  { x: -420, y: 132, rotate: 6, scale: 0.88 },
  { x: 420, y: 140, rotate: -6, scale: 0.88 },
  { x: -430, y: 74, rotate: -3, scale: 0.82 },
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
      <mark
        key={index}
        className="rounded-xs bg-[#fff0b3] box-decoration-clone px-1 py-0.5 text-[#181613]"
      >
        {part}
      </mark>
    );
  });
}

function TestimonialTweetCard({
  testimonial,
  ariaLabel,
  className,
  style,
  onMoveToSide,
}: {
  testimonial: (typeof testimonials)[number];
  ariaLabel: string;
  className?: string;
  style?: CSSProperties;
  onMoveToSide: () => void;
}) {
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
      className={cn([
        "border-color-subtle absolute rounded-[3px] border bg-white p-5 text-left transition-[transform,box-shadow,opacity] duration-500 ease-[cubic-bezier(0.22,1,0.36,1)] select-none motion-reduce:transition-none sm:p-5",
        className,
      ])}
      style={style}
    >
      <figure className="flex h-full flex-col">
        <figcaption className="flex items-start justify-between gap-4">
          <div className="flex min-w-0 items-center gap-3">
            <img
              src={testimonial.avatar}
              alt={`${testimonial.author} profile photo`}
              className="size-12 rounded-full object-cover shadow-sm"
            />
            <div className="min-w-0">
              <p className="truncate text-sm font-semibold text-[#181613]">
                {testimonial.author}
              </p>
              <p className="truncate text-sm leading-5 text-[#756b5d]">
                @{testimonial.username}
              </p>
            </div>
          </div>

          <a
            href={testimonial.url}
            target="_blank"
            rel="noreferrer"
            aria-label={`View ${testimonial.author} post on X`}
            onClick={(event) => event.stopPropagation()}
            className="inline-flex size-9 shrink-0 items-center justify-center rounded-full text-[#181613] transition-colors hover:bg-[#f7f4ef]"
          >
            <Icon
              icon="simple-icons:x"
              width={15}
              height={15}
              aria-hidden="true"
            />
          </a>
        </figcaption>

        <blockquote className="flex flex-1 items-center justify-start py-3">
          <p className="text-left text-lg leading-[1.25] font-semibold text-balance text-[#181613]">
            {renderPullQuote(testimonial.quote)}
          </p>
        </blockquote>

        <p className="border-t border-[#ede7dc] pt-3 text-xs leading-5 text-[#756b5d]">
          {testimonialNameContext}
        </p>
      </figure>
    </article>
  );
}

const authCallbackSearchSchema = z.object({
  code: z.string().optional(),
  token_hash: z.string().optional(),
  type: z
    .enum([
      "email",
      "recovery",
      "magiclink",
      "signup",
      "invite",
      "email_change",
    ])
    .optional()
    .catch(undefined),
  flow: z.enum(["desktop", "web"]).optional().catch("desktop"),
  scheme: desktopSchemeSchema.optional().catch("hyprnote"),
  redirect: z.string().optional(),
  error: z.string().optional(),
  error_description: z.string().optional(),
});

export const Route = createFileRoute("/")({
  validateSearch: authCallbackSearchSchema,
  beforeLoad: ({ search }) => {
    const hasAuthCallback =
      !!search.code || !!search.error || (!!search.token_hash && !!search.type);

    if (!hasAuthCallback) {
      return;
    }

    const flow = search.flow ?? "desktop";
    const scheme = search.scheme ?? "hyprnote";

    throw redirect({
      to: "/auth/",
      search: {
        flow,
        scheme,
        code: search.code,
        token_hash: search.token_hash,
        type: search.type,
        redirect: search.redirect,
        error: search.error,
        error_description: search.error_description,
      } as any,
    });
  },
  component: Component,
  loader: async () => {
    const [githubStats, stargazers] = await Promise.all([
      getGitHubStats(),
      getStargazers(),
    ]);

    return {
      githubStars: githubStats.stars ?? 8466,
      githubStargazers: stargazers,
    };
  },
  head: () => ({
    links: [{ rel: "canonical", href: ANARLOG_SITE_URL }],
    scripts: [
      {
        type: "application/ld+json",
        children: JSON.stringify(
          getStructuredDataGraph([
            getOrganizationJsonLd(),
            getSoftwareApplicationJsonLd({
              description: ROOT_DESCRIPTION,
              featureList,
            }),
          ]),
        ),
      },
    ],
  }),
});

function Component() {
  const { githubStars, githubStargazers } = Route.useLoaderData();
  const formattedGithubStars = githubStars.toLocaleString("en-US");

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <AnnouncementBanner />

      <div className="mx-auto w-full max-w-[700px] px-5 pt-4 pb-8 md:px-8 md:pt-4 md:pb-12">
        <div className="min-w-0 text-center">
          <section className="pt-10 pb-2 md:pt-12 md:pb-4">
            <h1 className="font-hand mx-auto max-w-3xl text-5xl leading-[0.98] font-semibold tracking-normal text-balance md:text-7xl">
              AI notepad for private meetings.
            </h1>
            <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
              Jot notes during the call. Anarlog turns them into an editable
              summary.
            </p>
            <div className="mt-8 flex flex-wrap justify-center gap-x-5 gap-y-3 text-sm">
              <DownloadButton />
            </div>
            <HeroWorkflowDemo />
            <CredibilityLogoMarquee />
          </section>

          <PrivacySection />

          <TestimonialsSection />

          <OpenSourceSection
            formattedGithubStars={formattedGithubStars}
            stargazers={githubStargazers}
          />

          <PricingSection />

          <section id="manifesto" className="pt-28 pb-14 md:pt-32 md:pb-16">
            <article
              className="mx-auto max-w-3xl overflow-hidden rounded-[3px] border border-[#eadfce] bg-[#fffaf0] px-7 py-9 text-left shadow-[0_18px_50px_rgba(68,54,36,0.12)] sm:px-10 sm:py-12"
              style={{
                backgroundImage:
                  "linear-gradient(115deg, rgba(255, 250, 240, 0.9), rgba(246, 236, 218, 0.82)), url('/textures/crumpled-paper.png')",
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
                  <div className="flex -space-x-2">
                    {manifestoSigners.map((member) =>
                      member.links.twitter ? (
                        <a
                          key={member.id}
                          href={member.links.twitter}
                          target="_blank"
                          rel="noopener noreferrer"
                          aria-label={`${member.name} on X`}
                          className="block rounded-full transition-transform hover:-translate-y-0.5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#181613]"
                        >
                          <img
                            src={member.avatar}
                            alt=""
                            width={30}
                            height={30}
                            className="size-[30px] rounded-full object-cover"
                            decoding="async"
                            loading="lazy"
                          />
                        </a>
                      ) : (
                        <span
                          key={member.id}
                          aria-label={`${member.name} profile picture`}
                          className="block rounded-full"
                          role="img"
                        >
                          <img
                            src={member.avatar}
                            alt=""
                            width={30}
                            height={30}
                            className="size-[30px] rounded-full object-cover"
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

function PricingSection() {
  return (
    <section id="pricing" className="pt-24 pb-8 md:pt-28 md:pb-10">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          Simple pricing
        </h2>
        <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
          Start with local meeting notes for free. Upgrade when you want hosted
          transcription, AI, sync, and sharing.
        </p>
        <p className="mx-auto mt-3 max-w-2xl text-sm leading-6 text-[#756b5d]">
          Google Calendar access is read-only. It lets Anarlog show upcoming
          events and associate them with your personal notes; Anarlog cannot
          create, edit, or delete calendar events.
        </p>
      </div>

      <div className="relative left-1/2 mt-8 grid w-screen max-w-[760px] -translate-x-1/2 grid-cols-1 gap-4 px-5 text-left md:grid-cols-2 md:px-8">
        {MARKETING_PLAN_TIERS.map((plan) => (
          <PricingCard key={plan.id} plan={plan} />
        ))}
      </div>
    </section>
  );
}

function PricingCard({ plan }: { plan: MarketingPlanData }) {
  const visibleFeatures = plan.features.filter(
    (feature) => feature.included === true,
  );

  return (
    <article
      className={cn([
        "flex min-h-[30rem] flex-col rounded-[3px] border bg-white p-6 transition-all duration-200",
        plan.popular
          ? "border-[#181613]/30 shadow-[0_22px_60px_rgba(24,22,19,0.14)] ring-1 ring-[#181613]/10"
          : "border-neutral-200 opacity-[0.58] shadow-[0_10px_32px_rgba(24,22,19,0.05)] focus-within:opacity-100 focus-within:shadow-[0_16px_46px_rgba(24,22,19,0.08)] hover:opacity-100 hover:shadow-[0_16px_46px_rgba(24,22,19,0.08)]",
      ])}
    >
      <div className="flex items-start">
        <h3 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
          {plan.name}
        </h3>
      </div>

      <p className="mt-4 min-h-[4.5rem] text-sm leading-6 text-[#4f4940]">
        {plan.description}
      </p>

      <div className="mt-5 min-h-[4rem]">
        {plan.price ? (
          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
            <span className="font-hand text-5xl leading-none font-semibold text-[#181613]">
              ${plan.price.monthly}
            </span>
            <span className="text-sm text-[#756b5d]">/month</span>
            {plan.price.yearly != null ? (
              <span className="text-sm text-[#756b5d]">
                or ${plan.price.yearly}/year
              </span>
            ) : null}
          </div>
        ) : (
          <div className="flex items-baseline gap-2">
            <span className="font-hand text-5xl leading-none font-semibold text-[#181613]">
              $0
            </span>
            <span className="text-sm text-[#756b5d]">/month</span>
          </div>
        )}
      </div>

      <div className="mt-5">
        <PlanFeatureList features={visibleFeatures} dense />
      </div>

      <div className="mt-auto pt-6">
        <a
          href={appleSiliconDownloadUrl}
          className={cn([
            "flex h-11 w-full items-center justify-center rounded-full text-sm font-medium transition-all hover:scale-[102%] active:scale-[98%]",
            plan.popular
              ? "bg-[#181613] text-white hover:bg-[#4f4940]"
              : "bg-[#f4efe6] text-[#181613] hover:bg-[#eadfce]",
          ])}
        >
          {plan.price
            ? `Download and start your ${PRO_TRIAL_DAYS}-day Pro trial`
            : "Download for free"}
        </a>
      </div>
    </article>
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
        <a
          href={appleSiliconDownloadUrl}
          className="mt-8 inline-flex items-center gap-2 rounded-full bg-[#181613] px-5 py-3 text-sm font-medium text-white"
        >
          <Icon
            icon="simple-icons:apple"
            width={16}
            height={16}
            className="shrink-0"
            aria-hidden="true"
          />
          <span>Download for free</span>
        </a>
      </div>
    </section>
  );
}

type GitHubStargazer = {
  username: string;
  avatar: string;
};

function OpenSourceSection({
  formattedGithubStars,
  stargazers,
}: {
  formattedGithubStars: string;
  stargazers: GitHubStargazer[];
}) {
  const visibleStargazers = stargazers.slice(0, 24);

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
          className="mt-6 inline-flex items-center gap-2 rounded-full border border-neutral-300 bg-white px-5 py-3 text-sm font-medium text-neutral-900 transition-colors hover:border-neutral-400 hover:bg-neutral-100"
        >
          <Icon icon="mdi:github" width={18} height={18} aria-hidden="true" />
          <span>{formattedGithubStars} stars on GitHub</span>
        </a>
      </div>

      {visibleStargazers.length > 0 && (
        <div className="relative mt-8 overflow-hidden px-5 md:px-8">
          <div
            className="pointer-events-none absolute inset-y-0 left-0 z-10 w-12 bg-linear-to-r from-white to-transparent md:w-20"
            aria-hidden="true"
          />
          <div
            className="pointer-events-none absolute inset-y-0 right-0 z-10 w-12 bg-linear-to-l from-white to-transparent md:w-20"
            aria-hidden="true"
          />
          <div className="mx-auto grid max-w-[620px] grid-cols-6 gap-2 sm:grid-cols-12">
            {visibleStargazers.map((stargazer) => (
              <a
                key={stargazer.username}
                href={`https://github.com/${stargazer.username}`}
                target="_blank"
                rel="noopener noreferrer"
                className="group aspect-square overflow-hidden rounded-[4px] border border-neutral-200 bg-neutral-100 transition-transform hover:-translate-y-0.5 hover:border-neutral-400"
                aria-label={`${stargazer.username} on GitHub`}
                title={stargazer.username}
              >
                <img
                  src={stargazer.avatar}
                  alt=""
                  className="h-full w-full object-cover grayscale transition duration-200 group-hover:grayscale-0"
                  loading="lazy"
                  decoding="async"
                />
              </a>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function CredibilityLogoMarquee() {
  return (
    <section
      className="relative pt-0 pb-2 md:pb-0"
      aria-labelledby="credibility-heading"
    >
      <p className="sr-only">
        {credibilityLogos.map((logo) => logo.name).join(", ")}
      </p>
      <div
        className="pointer-events-none absolute -top-[4.4rem] left-1/2 z-20 h-20 w-[15rem] -translate-x-[160%] text-neutral-950 max-[899px]:hidden"
        aria-hidden="true"
      >
        <p className="absolute top-0 left-0 w-max -rotate-[3deg] font-['Reenie_Beanie','Patrick_Hand',cursive] text-[25px] leading-none font-normal whitespace-nowrap lg:text-[28px]">
          people love us at
        </p>
        <svg
          className="absolute top-[1.65rem] left-[1.15rem] h-[2.9rem] w-[4.65rem] rotate-[5deg] text-neutral-950"
          viewBox="0 0 74 46"
          fill="none"
        >
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

      <div className="relative left-1/2 w-screen -translate-x-1/2 overflow-hidden bg-white motion-reduce:overflow-visible">
        <div
          className="pointer-events-none absolute inset-y-0 left-0 z-10 w-16 bg-linear-to-r from-white to-transparent motion-reduce:hidden md:w-32"
          aria-hidden="true"
        />
        <div
          className="pointer-events-none absolute inset-y-0 right-0 z-10 w-16 bg-linear-to-l from-white to-transparent motion-reduce:hidden md:w-32"
          aria-hidden="true"
        />

        <div
          className="animate-scroll-left flex w-max items-center motion-reduce:mx-auto motion-reduce:w-full motion-reduce:max-w-6xl motion-reduce:animate-none motion-reduce:justify-center motion-reduce:px-6"
          style={{ animationDuration: "36s" }}
          aria-hidden="true"
        >
          {[0, 1].map((trackIndex) => (
            <div
              key={trackIndex}
              className={cn([
                "flex shrink-0 items-center gap-14 px-7 md:gap-20 md:px-10",
                trackIndex === 0 &&
                  "motion-reduce:w-full motion-reduce:shrink motion-reduce:flex-wrap motion-reduce:justify-center motion-reduce:gap-x-12 motion-reduce:gap-y-6 motion-reduce:px-0 md:motion-reduce:gap-x-16",
                trackIndex === 1 && "motion-reduce:hidden",
              ])}
            >
              {credibilityLogos.map((logo) => (
                <img
                  key={`${trackIndex}-${logo.name}`}
                  src={logo.src}
                  alt=""
                  className={cn([
                    "h-6 w-auto max-w-none object-contain opacity-65 grayscale",
                    logo.className,
                  ])}
                  draggable={false}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
      <h2
        id="credibility-heading"
        className="mt-1 font-['Reenie_Beanie','Patrick_Hand',cursive] text-[22px] leading-none font-normal text-neutral-950 min-[900px]:sr-only"
      >
        people love us at
      </h2>
    </section>
  );
}

function TestimonialsSection() {
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
    <section className="py-16 md:py-20">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          What people say
        </h2>
        <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
          It's clear they love Anarlog.
        </p>
      </div>

      <div className="relative left-1/2 mx-auto mt-8 h-[19rem] w-screen max-w-[980px] -translate-x-1/2 overflow-visible px-5 sm:h-[18rem]">
        <div className="absolute top-0 left-1/2 z-0 flex h-[15.5rem] w-[calc(100%-2.5rem)] max-w-[380px] -translate-x-1/2 flex-col items-center justify-center text-center sm:h-[13.5rem] sm:w-[380px] sm:translate-y-[34px]">
          <p className="font-hand text-3xl leading-none font-semibold text-[#181613] sm:text-4xl">
            Try for yourself.
          </p>
          <div className="mt-6 flex items-center justify-center">
            <a
              href={appleSiliconDownloadUrl}
              className="inline-flex items-center gap-2 rounded-full bg-[#181613] px-5 py-3 text-sm font-medium text-white transition-colors hover:bg-[#4f4940]"
            >
              Start using for free
              <ArrowRight size={16} strokeWidth={2.2} aria-hidden="true" />
            </a>
          </div>
        </div>

        <div className="sm:hidden">
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
                className={cn([
                  "top-0 left-1/2 h-[15.5rem] w-[calc(100%-2.5rem)] cursor-pointer shadow-[0_24px_60px_rgba(24,22,19,0.14)] hover:shadow-[0_30px_75px_rgba(24,22,19,0.16)]",
                  isMoved || remainingIndex > 0 ? "pointer-events-none" : "",
                ])}
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

        <div className="hidden sm:block">
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
                className={cn([
                  "top-0 left-1/2 h-[13.5rem] w-[380px] cursor-pointer",
                  !isMoved && remainingIndex === 0
                    ? "shadow-[0_24px_60px_rgba(24,22,19,0.14)] hover:shadow-[0_30px_75px_rgba(24,22,19,0.16)]"
                    : "shadow-[0_14px_36px_rgba(24,22,19,0.1)] hover:shadow-[0_18px_44px_rgba(24,22,19,0.13)]",
                ])}
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

function PrivacySection() {
  return (
    <section className="py-16 md:py-20">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          Your data stays yours
        </h2>
        <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
          Anarlog is built around data you own, privacy you control, and notes
          that stay useful outside our app.
        </p>
      </div>

      <div className="relative left-1/2 mt-6 w-screen max-w-[1120px] -translate-x-1/2">
        <div className="grid gap-4 md:flex md:items-start md:justify-between md:gap-0">
          {privacyCommitments.map((commitment) => {
            return (
              <div
                key={commitment.description}
                className="flex flex-col px-6 py-3 text-center md:w-[31%] md:p-4"
              >
                <PrivacyVisual type={commitment.visual} />
                <p className="mx-auto mt-3 max-w-[15rem] text-sm leading-6 text-[#4f4940] md:mt-5">
                  {commitment.description}
                </p>
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
    return (
      <div className="flex h-20 items-center justify-center gap-2 select-none md:h-28 md:w-full md:justify-between md:gap-1">
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[3deg] object-contain transition-transform duration-300 ease-out hover:rotate-[7deg]"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[-5deg] object-contain transition-transform duration-300 ease-out hover:rotate-[-9deg]"
          draggable={false}
        />
        <img
          src="/icons/folderchar.svg"
          alt=""
          className="w-14 object-contain transition-transform duration-300 ease-out hover:rotate-[3deg]"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[6deg] object-contain transition-transform duration-300 ease-out hover:rotate-[10deg]"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[-4deg] object-contain transition-transform duration-300 ease-out hover:rotate-[-8deg]"
          draggable={false}
        />
      </div>
    );
  }

  if (type === "key") {
    return (
      <div className="flex h-24 items-center justify-center select-none md:h-28 md:w-full">
        <div
          className="relative h-24 w-52 md:h-28 md:w-60"
          role="img"
          aria-label="AI option cards cycling between cloud, key, and chip"
        >
          <AiOptionPlayingCard
            className="ai-option-card-cloud ai-option-card-red"
            rank="C"
            IconComponent={Cloud}
          />
          <AiOptionPlayingCard
            className="ai-option-card-key"
            rank="K"
            IconComponent={KeyRound}
          />
          <AiOptionPlayingCard
            className="ai-option-card-chip"
            rank="O"
            IconComponent={Cpu}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-20 items-center justify-center select-none md:h-28 md:w-full">
      <div className="flex w-full max-w-[260px] items-center gap-3 rounded-2xl border border-neutral-200 bg-white py-2 pr-3 pl-4 text-left shadow-[0_3px_10px_rgba(24,22,19,0.04)]">
        <img
          src="/icons/google-meet.svg"
          alt=""
          className="h-7 w-7 object-contain"
          draggable={false}
        />
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-stone-800">
            Sprint 3 planning
          </span>
          <span className="text-sm text-stone-400">5 participants</span>
        </div>
        <div
          className="meeting-audio-bars ml-auto flex h-6 items-center gap-0.5"
          aria-hidden="true"
        >
          <span className="meeting-audio-bar" />
          <span className="meeting-audio-bar" />
          <span className="meeting-audio-bar" />
        </div>
      </div>
    </div>
  );
}

function AiOptionPlayingCard({
  className,
  rank,
  IconComponent,
}: {
  className: string;
  rank: string;
  IconComponent: LucideIcon;
}) {
  return (
    <div className={cn(["ai-option-card", className])}>
      <span className="ai-option-card-corner ai-option-card-corner-top">
        <span className="ai-option-card-rank">{rank}</span>
      </span>
      <div className="ai-option-card-face">
        <IconComponent aria-hidden="true" />
      </div>
      <span className="ai-option-card-corner ai-option-card-corner-bottom">
        <span className="ai-option-card-rank">{rank}</span>
      </span>
    </div>
  );
}

function HeroWorkflowDemo() {
  const [typedText1, setTypedText1] = useState("");
  const [typedText2, setTypedText2] = useState("");
  const [enhancedLines, setEnhancedLines] = useState(0);
  const [isTypingActive, setIsTypingActive] = useState(false);

  const text1 = "metrisc w/ john";
  const text2 = "stakehlder mtg";

  useMountEffect(() => {
    const timeouts: ReturnType<typeof setTimeout>[] = [];
    const intervals: ReturnType<typeof setInterval>[] = [];

    const queueTimeout = (callback: () => void, delay: number) => {
      const timeout = setTimeout(callback, delay);
      timeouts.push(timeout);
    };

    const runAnimation = () => {
      setTypedText1("");
      setTypedText2("");
      setEnhancedLines(0);
      setIsTypingActive(false);

      let currentIndex1 = 0;
      queueTimeout(() => {
        setIsTypingActive(true);
        const interval1 = setInterval(() => {
          if (currentIndex1 < text1.length) {
            setTypedText1(text1.slice(0, currentIndex1 + 1));
            currentIndex1++;
          } else {
            clearInterval(interval1);

            let currentIndex2 = 0;
            const interval2 = setInterval(() => {
              if (currentIndex2 < text2.length) {
                setTypedText2(text2.slice(0, currentIndex2 + 1));
                currentIndex2++;
              } else {
                clearInterval(interval2);
                setIsTypingActive(false);

                queueTimeout(() => {
                  setEnhancedLines(1);
                  queueTimeout(() => {
                    setEnhancedLines(2);
                    queueTimeout(() => {
                      setEnhancedLines(3);
                      queueTimeout(() => {
                        setEnhancedLines(4);
                        queueTimeout(() => {
                          setEnhancedLines(5);
                          queueTimeout(() => {
                            setEnhancedLines(6);
                            queueTimeout(() => runAnimation(), 3000);
                          }, 800);
                        }, 800);
                      }, 800);
                    }, 800);
                  }, 800);
                }, 500);
              }
            }, 50);
            intervals.push(interval2);
          }
        }, 50);
        intervals.push(interval1);
      }, 500);
    };

    runAnimation();

    return () => {
      timeouts.forEach(clearTimeout);
      intervals.forEach(clearInterval);
    };
  });

  const isSummaryPhase = enhancedLines > 0;
  const isGeneratingSummary = enhancedLines > 0 && enhancedLines < 6;

  return (
    <div className="relative left-1/2 mt-10 w-screen max-w-[500px] -translate-x-1/2 px-8 sm:px-10">
      <div
        className="pointer-events-none absolute top-10 bottom-24 left-8 z-0 w-12 rounded-full bg-neutral-950/10 blur-2xl sm:left-10"
        aria-hidden="true"
      />
      <div
        className="pointer-events-none absolute top-10 right-8 bottom-24 z-0 w-12 rounded-full bg-neutral-950/10 blur-2xl sm:right-10"
        aria-hidden="true"
      />
      <div
        className="relative z-10 mx-auto max-w-[420px] overflow-hidden rounded-xl border-x border-t border-neutral-200 bg-white shadow-[0_24px_70px_rgba(24,22,19,0.08)]"
        style={{
          WebkitMaskImage:
            "linear-gradient(to bottom, black 0%, black calc(100% - 5rem), transparent 100%)",
          maskImage:
            "linear-gradient(to bottom, black 0%, black calc(100% - 5rem), transparent 100%)",
        }}
      >
        <div className="flex items-center gap-2 px-4 py-3">
          <div className="flex gap-2">
            <div className="h-3 w-3 rounded-full bg-red-400"></div>
            <div className="h-3 w-3 rounded-full bg-yellow-400"></div>
            <div className="h-3 w-3 rounded-full bg-green-400"></div>
          </div>
          <div className="ml-auto flex h-4 w-6 items-center justify-end">
            {isGeneratingSummary ? (
              <Spinner size={12} className="text-neutral-500" />
            ) : !isSummaryPhase ? (
              <DancingSticks
                amplitude={isTypingActive ? 1 : 0}
                height={12}
                color="#f87171"
              />
            ) : null}
          </div>
        </div>
        <div className="relative min-h-[260px] overflow-hidden text-left text-sm sm:min-h-[300px]">
          <div
            className={cn([
              "absolute inset-0 space-y-3 p-5 transition-opacity duration-500 sm:p-6",
              isSummaryPhase ? "opacity-0" : "opacity-100",
            ])}
          >
            <div className="text-neutral-700">ui update - moble</div>
            <div className="text-neutral-700">api</div>
            <div className="mt-4 text-neutral-700">new dash - urgnet</div>
            <div className="text-neutral-700">a/b tst next wk</div>
            <div className="mt-4 min-h-5 text-neutral-700">
              {typedText1}
              <span
                className={cn([
                  typedText1 && typedText1.length < text1.length
                    ? "animate-pulse"
                    : "opacity-0",
                ])}
              >
                |
              </span>
            </div>
            <div className="min-h-5 text-neutral-700">
              {typedText2}
              <span
                className={cn([
                  typedText2 && typedText2.length < text2.length
                    ? "animate-pulse"
                    : "opacity-0",
                ])}
              >
                |
              </span>
            </div>
          </div>
          <div
            className={cn([
              "absolute inset-0 space-y-4 overflow-hidden p-5 text-left transition-opacity duration-500 sm:p-6",
              isSummaryPhase ? "opacity-100" : "opacity-0",
            ])}
          >
            <div className="space-y-2">
              <h4
                className={cn([
                  "font-semibold text-stone-700 transition-opacity duration-500",
                  enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                ])}
              >
                Mobile UI Update and API Adjustments
              </h4>
              <ul className="list-disc space-y-2 pl-5 text-sm text-neutral-700">
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Sarah presented the new mobile UI update, which includes a
                  streamlined navigation bar and improved button placements for
                  better accessibility.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 2 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Ben confirmed that API adjustments are needed to support
                  dynamic UI changes, particularly for fetching personalized
                  user data more efficiently.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 3 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  The UI update will be implemented in phases, starting with
                  core navigation improvements. Ben will ensure API
                  modifications are completed before development begins.
                </li>
              </ul>
            </div>
            <div className="space-y-2">
              <h4
                className={cn([
                  "font-semibold text-stone-700 transition-opacity duration-500",
                  enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                ])}
              >
                New Dashboard - Urgent Priority
              </h4>
              <ul className="list-disc space-y-2 pl-5 text-sm text-neutral-700">
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Alice emphasized that the new analytics dashboard must be
                  prioritized due to increasing stakeholder demand.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  The new dashboard will feature real-time user engagement
                  metrics and a customizable reporting system.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Ben mentioned that backend infrastructure needs optimization
                  to handle real-time data processing.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Mark stressed that the dashboard launch should align with
                  marketing efforts to maximize user adoption.
                </li>
                <li
                  className={cn([
                    "transition-opacity duration-500",
                    enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                  ])}
                >
                  Development will start immediately, and a basic prototype must
                  be ready for stakeholder review next week.
                </li>
              </ul>
            </div>
          </div>
        </div>
      </div>
      <div
        className="pointer-events-none absolute right-0 bottom-0 left-0 z-10 h-28 bg-linear-to-t from-white to-transparent"
        aria-hidden="true"
      />
    </div>
  );
}

function AnnouncementBanner() {
  return (
    <div className="flex justify-center px-5 pt-10 md:pt-12">
      <a
        href="https://char.com"
        className="char-announcement group relative inline-flex h-8 w-[15.25rem] max-w-full items-center justify-center text-center text-sm font-medium text-[#181613] opacity-75 transition-opacity hover:opacity-100"
        aria-label="Join the waitlist for Char"
      >
        <span className="char-announcement-text char-announcement-text-primary absolute inset-y-0 flex min-w-0 items-center justify-center px-8 whitespace-nowrap">
          Join the waitlist for Char
        </span>
        <span className="char-announcement-text char-announcement-text-secondary absolute inset-y-0 flex min-w-0 items-center justify-center px-8 whitespace-nowrap">
          We're innovating the todo list
        </span>
        <span
          className="pointer-events-none absolute inset-0"
          aria-hidden="true"
        >
          <CharBracket
            side="left"
            className="char-announcement-bracket char-announcement-bracket-left absolute top-1/2 left-1/2 h-5 text-[#181613]"
          />
          <CharBracket
            side="right"
            className="char-announcement-bracket char-announcement-bracket-right absolute top-1/2 left-1/2 h-5 text-[#181613]"
          />
        </span>
      </a>
    </div>
  );
}

function CharBracket({
  side,
  className,
}: {
  side: "left" | "right";
  className?: string;
}) {
  return (
    <svg
      width="8"
      height="30"
      viewBox="0 0 8 30"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      aria-hidden="true"
    >
      {side === "left" ? (
        <path
          d="M7.871 4.147C7.871 5.658 7.082 7.039 6.099 8.214C4.65 9.946 3.77 12.161 3.77 14.575C3.77 16.99 4.65 19.205 6.099 20.937C7.082 22.112 7.871 23.493 7.871 25.004V29.151H2.965V24.319C2.965 22.735 2.165 21.249 0.822 20.34L0 19.783V9.235L0.822 8.678C2.165 7.769 2.965 6.284 2.965 4.699V0L7.871 0V4.147Z"
          fill="currentColor"
        />
      ) : (
        <path
          d="M0 4.147C0 5.658 0.789 7.039 1.773 8.214C3.221 9.946 4.101 12.161 4.101 14.575C4.101 16.99 3.221 19.205 1.773 20.937C0.789 22.112 0 23.493 0 25.004V29.151H4.907V24.319C4.907 22.735 5.706 21.249 7.049 20.34L7.871 19.783V9.235L7.049 8.678C5.706 7.769 4.907 6.284 4.907 4.699V0L0 0V4.147Z"
          fill="currentColor"
        />
      )}
    </svg>
  );
}

function DownloadButton() {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;

    const onPointerDown = (event: MouseEvent | TouchEvent) => {
      if (!containerRef.current) return;
      if (!containerRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };

    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("touchstart", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("touchstart", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div
      ref={containerRef}
      className="relative inline-flex text-sm font-medium"
    >
      <a
        href={appleSiliconDownloadUrl}
        className="inline-flex items-center gap-1 rounded-l-full bg-[#181613] py-3 pr-1 pl-4 text-[13px] text-white sm:pl-5 sm:text-sm"
      >
        <Icon
          icon="simple-icons:apple"
          width={16}
          height={16}
          className="shrink-0"
          aria-hidden="true"
        />
        <span>Download for Apple Silicon</span>
      </a>
      <button
        type="button"
        aria-label="Choose download platform"
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={() => setOpen((prev) => !prev)}
        className="inline-flex h-full cursor-pointer items-center rounded-r-full bg-[#181613] py-3 pr-3 pl-2 text-white"
      >
        <ChevronDown size={17} strokeWidth={2.2} aria-hidden="true" />
      </button>
      {open && (
        <div
          role="menu"
          className="absolute top-[calc(100%+0.5rem)] left-0 z-10 w-full max-w-[calc(100vw-2.5rem)] rounded-2xl border border-[#d8d0c5] bg-white p-2 shadow-[0_14px_40px_rgba(24,22,19,0.12)]"
        >
          <a
            href={appleIntelDownloadUrl}
            className="flex items-center gap-3 rounded-xl px-3 py-3 text-[#181613] transition-colors hover:bg-[#f7f4ef]"
          >
            <Icon
              icon="simple-icons:apple"
              width={20}
              height={20}
              className="shrink-0"
              aria-hidden="true"
            />
            <span>Apple Intel</span>
          </a>
        </div>
      )}
    </div>
  );
}
