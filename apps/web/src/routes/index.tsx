import { Icon } from "@iconify-icon/react";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { ArrowRight, ChevronDown, KeyRound, WifiOff } from "lucide-react";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import { z } from "zod";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import { SiteFooter } from "@/components/site-footer";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getGitHubStats } from "@/functions/github";
import { useMountEffect } from "@/hooks/useMountEffect";
import {
  ANARLOG_SITE_URL,
  ROOT_DESCRIPTION,
  getOrganizationJsonLd,
  getSoftwareApplicationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

const manifestoLetter = [
  "To the people who still take notes,",
  "Notetaking matters more than note-takers. A note-taker is passive. A notepad is something you use. You stay present and in control while the room is still alive.",
  "Most AI tools ask you to move your memory into their ecosystem and rules. Meeting notes should move the other way: back to files on your disk and software you can run offline.",
  "Files endure. Interfaces change. Your notes should survive us. Use on-device models or your own keys, not a service you cannot inspect.",
  "Anarlog is our attempt to build that meeting notepad.",
  "John Jeong, Yujong Lee",
];

const featureList = [
  "Bot-free meeting capture",
  "Fully offline notes",
  "On-device or bring-your-own-key AI",
  "File-based storage",
  "Open source foundations",
];

const privacyCommitments = [
  {
    title: "Your notes stay yours",
    description: "Audio, transcripts, and notes live as files on your device.",
    visual: "files",
  },
  {
    title: "Choose the AI path",
    description: "Use local models, or bring your own key when cloud AI fits.",
    visual: "key",
  },
  {
    title: "No bots on calls",
    description: "Capture system audio without adding a bot to the meeting.",
    visual: "meeting",
  },
];

const credibilityLogos = [
  { name: "Databricks", src: "/icons/databricks.svg" },
  { name: "Amazon", src: "/icons/amazon.svg", className: "max-h-9" },
  { name: "Meta", src: "/icons/meta.svg", className: "max-h-9" },
  { name: "Y Combinator", src: "/icons/yc.svg" },
  { name: "Palantir", src: "/icons/palantir.svg" },
  { name: "Apple", src: "/icons/apple.svg", className: "max-h-9" },
  { name: "Disney", src: "/icons/disney.svg", className: "max-h-9" },
  { name: "Richmond American", src: "/icons/richmond_american.svg" },
  { name: "Adobe", src: "/icons/adobe.svg", className: "max-h-8" },
  { name: "Wayfair", src: "/icons/wayfair.svg" },
  { name: "Bain & Company", src: "/icons/bain.svg", className: "max-h-6" },
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
        "border-color-subtle absolute rounded-lg border bg-white p-5 transition-[transform,box-shadow,opacity] duration-500 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none sm:p-5",
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

        <blockquote className="flex flex-1 items-center py-3">
          <p className="text-lg leading-[1.25] font-semibold text-balance text-[#181613]">
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

const appleSiliconDownloadUrl =
  "https://cdn.crabnebula.app/download/fastrepl/hyprnote2/latest/platform/dmg-aarch64?channel=stable";
const appleIntelDownloadUrl =
  "https://cdn.crabnebula.app/download/fastrepl/hyprnote2/latest/platform/dmg-x86_64?channel=stable";

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
  loader: async () => ({
    githubStars: (await getGitHubStats()).stars ?? 8466,
  }),
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
  const { githubStars } = Route.useLoaderData();
  const formattedGithubStars = githubStars.toLocaleString("en-US");

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <AnnouncementBanner />

      <div className="mx-auto w-full max-w-[700px] px-5 pt-4 pb-8 md:px-8 md:pt-4 md:pb-12">
        <div className="min-w-0">
          <section className="pt-10 pb-16 md:pt-12">
            <h1 className="font-hand max-w-3xl text-6xl leading-[0.98] font-semibold tracking-normal text-balance md:text-8xl">
              AI notepad for private meetings.
            </h1>
            <p className="mt-6 max-w-2xl text-xl leading-9 text-[#363029]">
              The open-source Granola alternative for private meetings,
              downloaded over 11.9M times. Record locally and choose where AI
              runs.
            </p>
            <div className="mt-8 flex flex-wrap gap-x-5 gap-y-3 text-sm">
              <DownloadButton />
              <a
                href="https://github.com/fastrepl/anarlog"
                className="inline-flex items-center gap-2 rounded-full border border-[#d8d0c5] px-5 py-3 font-medium text-[#181613] transition-colors hover:border-[#b8aea0] hover:bg-[#f7f4ef]"
              >
                <img
                  src="https://upload.wikimedia.org/wikipedia/commons/9/91/Octicons-mark-github.svg"
                  alt=""
                  className="size-4"
                  aria-hidden="true"
                />
                <span>GitHub</span>
                <span className="text-[#756b5d]">
                  {formattedGithubStars} stars
                </span>
              </a>
            </div>
          </section>

          <CredibilityLogoMarquee />

          <HowItWorksSection />

          <PrivacySection />

          <TestimonialsSection />

          <section id="manifesto" className="py-10">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Manifesto
            </h2>
            <div className="mt-7 max-w-3xl">
              <div className="space-y-6 text-lg leading-8 text-[#363029]">
                {manifestoLetter.slice(0, -1).map((paragraph) => (
                  <p key={paragraph}>{paragraph}</p>
                ))}
              </div>
              <div className="mt-10">
                <p className="font-signature text-3xl leading-none font-normal">
                  {manifestoLetter.at(-1)}
                </p>
                <p className="mt-5 font-sans text-base leading-none font-normal text-[#4f4940]">
                  Fastrepl, Inc.
                </p>
              </div>
            </div>
          </section>
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}

function CredibilityLogoMarquee() {
  return (
    <section className="py-8" aria-labelledby="credibility-heading">
      <h2
        id="credibility-heading"
        className="font-hand text-3xl leading-none font-semibold text-[#756b5d]"
      >
        Trusted by people in
      </h2>
      <p className="sr-only">
        {credibilityLogos.map((logo) => logo.name).join(", ")}
      </p>

      <div className="relative left-1/2 mt-6 w-screen -translate-x-1/2 overflow-hidden bg-white py-4 motion-reduce:overflow-visible">
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
                    "h-7 w-auto max-w-none object-contain opacity-65 grayscale",
                    logo.className,
                  ])}
                  draggable={false}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
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
    <section className="pt-8 pb-2">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          What people say
        </h2>
        <p className="text-color-secondary mt-6 text-lg leading-8">
          See for yourself. Before Anarlog had this name, people kept pointing
          at the same things: private meeting notes, no bot in the room, and
          local control by default.
        </p>
      </div>

      <div className="relative left-1/2 mx-auto mt-8 h-[19rem] w-screen max-w-[980px] -translate-x-1/2 overflow-visible px-5 sm:h-[18rem]">
        <div className="absolute top-0 left-1/2 z-0 flex h-[15.5rem] w-[calc(100%-2.5rem)] max-w-[380px] -translate-x-1/2 flex-col items-center justify-center text-center sm:h-[13.5rem] sm:w-[380px] sm:translate-y-[34px]">
          <p className="font-hand text-3xl leading-none font-semibold text-[#756b5d] sm:text-4xl">
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
    <section className="py-10">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          What makes it different
        </h2>
        <p className="mt-6 text-lg leading-8 text-[#4f4940]">
          Anarlog stays out of the participant list, keeps notes on disk, and
          lets you pick the AI path.
        </p>
      </div>

      <div className="relative left-1/2 mt-6 w-screen max-w-[1120px] -translate-x-1/2">
        <div className="grid gap-4 md:flex md:items-start md:justify-between md:gap-0">
          {privacyCommitments.map((commitment) => {
            return (
              <div
                key={commitment.title}
                className="flex flex-col px-6 py-3 md:w-[31%] md:p-4"
              >
                <PrivacyVisual type={commitment.visual} />
                <h3 className="mt-3 text-sm font-semibold text-[#181613] md:mt-5">
                  {commitment.title}
                </h3>
                <p className="mt-2 text-sm leading-6 text-[#4f4940]">
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
      <div className="flex h-20 items-center justify-start gap-2 select-none md:h-28 md:w-full md:justify-between md:gap-1">
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[3deg] object-contain"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[-5deg] object-contain"
          draggable={false}
        />
        <img
          src="/icons/folderchar.svg"
          alt=""
          className="w-14 object-contain"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[6deg] object-contain"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[-4deg] object-contain"
          draggable={false}
        />
      </div>
    );
  }

  if (type === "key") {
    return (
      <div className="flex h-20 items-center gap-4 select-none md:h-28 md:w-full">
        <WifiOff className="size-6 shrink-0 text-[#756b5d]" />
        <div className="relative flex min-w-0 flex-1 items-center overflow-hidden rounded-lg border border-neutral-200 px-3 py-4">
          <KeyRound className="mr-2 size-4 shrink-0 text-stone-400" />
          <span className="text-base tracking-wider text-stone-300">sk-</span>
          <span className="text-base tracking-[0.2em] text-stone-400">
            ***************
          </span>
          <div className="pointer-events-none absolute inset-y-0 right-0 w-16 bg-linear-to-l from-white to-transparent" />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-20 items-center select-none md:h-28 md:w-full">
      <div className="flex items-center gap-3 rounded-2xl border border-neutral-200 bg-white py-2 pr-8 pl-4 shadow-lg md:w-full">
        <Icon icon="logos:google-meet" width={32} height={32} />
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-stone-800">
            Sprint 3 planning
          </span>
          <span className="text-sm text-stone-400">5 participants</span>
        </div>
      </div>
    </div>
  );
}

function HowItWorksSection() {
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

  return (
    <section className="py-10">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          How it works
        </h2>
        <p className="mt-6 text-lg leading-8 text-[#4f4940]">
          Jot notes during the call. Anarlog turns them into an editable
          summary.
        </p>
      </div>
      <div className="relative left-1/2 mt-8 hidden w-screen max-w-[1120px] -translate-x-1/2 sm:grid sm:grid-cols-2">
        <div
          className="pointer-events-none absolute right-0 bottom-0 left-0 z-10 h-36 bg-linear-to-t from-white to-transparent"
          aria-hidden="true"
        />
        <div className="flex flex-col overflow-clip">
          <div className="flex flex-col gap-4 p-8">
            <p className="text-sm leading-6 text-neutral-600">
              <span className="font-semibold">Take notes.</span> Anarlog records
              from your device. No bot joins.
            </p>
          </div>
          <div className="flex flex-1 items-center justify-center bg-stone-50/30 px-8 pb-0">
            <div className="w-full max-w-lg overflow-hidden rounded-t-xl border border-b-0 border-neutral-200 bg-white shadow-lg">
              <div className="flex items-center gap-2 px-4 py-3">
                <div className="flex gap-2">
                  <div className="h-3 w-3 rounded-full bg-red-400"></div>
                  <div className="h-3 w-3 rounded-full bg-yellow-400"></div>
                  <div className="h-3 w-3 rounded-full bg-green-400"></div>
                </div>
                <div className="ml-auto">
                  <DancingSticks
                    amplitude={isTypingActive ? 1 : 0}
                    height={12}
                    color="#a3a3a3"
                  />
                </div>
              </div>
              {/* Content area */}
              <div className="min-h-[300px] space-y-3 p-6 text-sm">
                <div className="text-neutral-700">ui update - moble</div>
                <div className="text-neutral-700">api</div>
                <div className="mt-4 text-neutral-700">new dash - urgnet</div>
                <div className="text-neutral-700">a/b tst next wk</div>
                <div className="mt-4 text-neutral-700">
                  {typedText1}
                  {typedText1 && typedText1.length < text1.length && (
                    <span className="animate-pulse">|</span>
                  )}
                </div>
                <div className="text-neutral-700">
                  {typedText2}
                  {typedText2 && typedText2.length < text2.length && (
                    <span className="animate-pulse">|</span>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className="flex flex-col overflow-clip">
          <div className="flex flex-col gap-4 p-8">
            <p className="text-sm leading-6 text-neutral-600">
              <span className="font-semibold">After the call,</span> your notes
              become an editable summary.
            </p>
          </div>
          <div className="flex flex-1 items-start justify-center bg-stone-50/30 px-8 pb-0">
            <div className="w-full max-w-lg overflow-hidden rounded-t-xl border border-b-0 border-neutral-200 bg-white shadow-lg">
              <div className="flex items-center gap-2 px-4 py-3">
                <div className="flex gap-2">
                  <div className="h-3 w-3 rounded-full bg-red-400"></div>
                  <div className="h-3 w-3 rounded-full bg-yellow-400"></div>
                  <div className="h-3 w-3 rounded-full bg-green-400"></div>
                </div>
              </div>
              {/* Content area */}
              <div className="max-h-[300px] min-h-[300px] space-y-4 overflow-hidden p-6">
                <div className="space-y-2">
                  <h4
                    className={cn(
                      "font-semibold text-stone-700 transition-opacity duration-500",
                      enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                    )}
                  >
                    Mobile UI Update and API Adjustments
                  </h4>
                  <ul className="list-disc space-y-2 pl-5 text-sm text-neutral-700">
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Sarah presented the new mobile UI update, which includes a
                      streamlined navigation bar and improved button placements
                      for better accessibility.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 2 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Ben confirmed that API adjustments are needed to support
                      dynamic UI changes, particularly for fetching personalized
                      user data more efficiently.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 3 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      The UI update will be implemented in phases, starting with
                      core navigation improvements. Ben will ensure API
                      modifications are completed before development begins.
                    </li>
                  </ul>
                </div>
                <div className="space-y-2">
                  <h4
                    className={cn(
                      "font-semibold text-stone-700 transition-opacity duration-500",
                      enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                    )}
                  >
                    New Dashboard – Urgent Priority
                  </h4>
                  <ul className="list-disc space-y-2 pl-5 text-sm text-neutral-700">
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Alice emphasized that the new analytics dashboard must be
                      prioritized due to increasing stakeholder demand.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      The new dashboard will feature real-time user engagement
                      metrics and a customizable reporting system.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 6 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Ben mentioned that backend infrastructure needs
                      optimization to handle real-time data processing.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 6 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Mark stressed that the dashboard launch should align with
                      marketing efforts to maximize user adoption.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 6 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Development will start immediately, and a basic prototype
                      must be ready for stakeholder review next week.
                    </li>
                  </ul>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="relative left-1/2 mt-8 w-screen max-w-[1120px] -translate-x-1/2 sm:hidden">
        <div
          className="pointer-events-none absolute right-0 bottom-0 left-0 z-10 h-28 bg-linear-to-t from-white to-transparent"
          aria-hidden="true"
        />
        <div>
          <div className="p-6">
            <p className="mb-4 text-sm leading-6 text-neutral-600">
              <span className="font-semibold">Take notes.</span> Anarlog records
              from your device. No bot joins.
            </p>
          </div>
          <div className="relative overflow-clip bg-stone-50/30 px-6 pb-0">
            <div
              className="pointer-events-none absolute right-0 bottom-0 left-0 z-10 h-20 bg-linear-to-t from-white to-transparent"
              aria-hidden="true"
            />
            <div className="overflow-hidden rounded-t-lg border border-b-0 border-neutral-200 bg-white shadow-lg">
              <div className="flex items-center gap-1.5 px-3 py-2">
                <div className="flex gap-1.5">
                  <div className="h-2 w-2 rounded-full bg-red-400"></div>
                  <div className="h-2 w-2 rounded-full bg-yellow-400"></div>
                  <div className="h-2 w-2 rounded-full bg-green-400"></div>
                </div>
                <div className="ml-auto">
                  <DancingSticks
                    amplitude={isTypingActive ? 1 : 0}
                    height={10}
                    color="#a3a3a3"
                  />
                </div>
              </div>
              <div className="min-h-[200px] space-y-2 p-4 text-xs">
                <div className="text-neutral-700">ui update - moble</div>
                <div className="text-neutral-700">api</div>
                <div className="mt-3 text-neutral-700">new dash - urgnet</div>
                <div className="text-neutral-700">a/b tst next wk</div>
                <div className="mt-3 text-neutral-700">
                  {typedText1}
                  {typedText1 && typedText1.length < text1.length && (
                    <span className="animate-pulse">|</span>
                  )}
                </div>
                <div className="text-neutral-700">
                  {typedText2}
                  {typedText2 && typedText2.length < text2.length && (
                    <span className="animate-pulse">|</span>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>

        <div>
          <div className="p-6">
            <p className="mb-4 text-sm leading-6 text-neutral-600">
              <span className="font-semibold">After the call,</span> your notes
              become an editable summary.
            </p>
          </div>
          <div className="overflow-clip bg-stone-50/30 px-6 pb-0">
            <div className="overflow-hidden rounded-t-lg border border-b-0 border-neutral-200 bg-white shadow-lg">
              <div className="flex items-center gap-1.5 px-3 py-2">
                <div className="flex gap-1.5">
                  <div className="h-2 w-2 rounded-full bg-red-400"></div>
                  <div className="h-2 w-2 rounded-full bg-yellow-400"></div>
                  <div className="h-2 w-2 rounded-full bg-green-400"></div>
                </div>
              </div>
              <div className="max-h-[200px] min-h-[200px] space-y-3 overflow-hidden p-4">
                <div className="space-y-2">
                  <h4 className="text-sm font-semibold text-stone-700">
                    Mobile UI Update and API Adjustments
                  </h4>
                  <ul className="list-disc space-y-2 pl-4 text-xs text-neutral-700">
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 1 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Sarah presented the new mobile UI update, which includes a
                      streamlined navigation bar and improved button placements
                      for better accessibility.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 2 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Ben confirmed that API adjustments are needed to support
                      dynamic UI changes, particularly for fetching personalized
                      user data more efficiently.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 3 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      The UI update will be implemented in phases, starting with
                      core navigation improvements. Ben will ensure API
                      modifications are completed before development begins.
                    </li>
                  </ul>
                </div>
                <div className="space-y-2">
                  <h4 className="text-sm font-semibold text-stone-700">
                    New Dashboard – Urgent Priority
                  </h4>
                  <ul className="list-disc space-y-2 pl-4 text-xs text-neutral-700">
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 4 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Alice emphasized that the new analytics dashboard must be
                      prioritized due to increasing stakeholder demand.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 5 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      The new dashboard will feature real-time user engagement
                      metrics and a customizable reporting system.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 6 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Ben mentioned that backend infrastructure needs
                      optimization to handle real-time data processing.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 6 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Mark stressed that the dashboard launch should align with
                      marketing efforts to maximize user adoption.
                    </li>
                    <li
                      className={cn(
                        "transition-opacity duration-500",
                        enhancedLines >= 6 ? "opacity-100" : "opacity-0",
                      )}
                    >
                      Development will start immediately, and a basic prototype
                      must be ready for stakeholder review next week.
                    </li>
                  </ul>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function AnnouncementBanner() {
  return (
    <div className="flex justify-center px-5 pt-6 md:pt-8">
      <a
        href="https://char.com"
        className="border-color-subtle text-color group inline-flex max-w-full items-center justify-center gap-2 rounded-full border bg-white px-4 py-2 text-center text-sm font-medium shadow-sm transition-colors hover:bg-neutral-50 md:px-5"
        aria-label="Visit Char v2"
      >
        <span className="min-w-0">See what we're building next</span>
        <ArrowRight
          size={16}
          strokeWidth={2.2}
          className="shrink-0 transition-transform group-hover:translate-x-0.5"
          aria-hidden="true"
        />
      </a>
    </div>
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
        <img
          src="https://upload.wikimedia.org/wikipedia/commons/f/fa/Apple_logo_black.svg"
          alt=""
          className="size-4 invert"
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
            <img
              src="https://upload.wikimedia.org/wikipedia/commons/f/fa/Apple_logo_black.svg"
              alt=""
              className="size-5"
              aria-hidden="true"
            />
            <span>Apple Intel</span>
          </a>
        </div>
      )}
    </div>
  );
}
