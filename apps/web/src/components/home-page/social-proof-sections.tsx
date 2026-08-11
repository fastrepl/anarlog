import { Icon } from "@iconify-icon/react";
import { ArrowRight } from "@phosphor-icons/react";
import { Link } from "@tanstack/react-router";
import { type CSSProperties, useState } from "react";

import { cn } from "@anlg/utils";

import { getResizedImageSrcSet, getResizedImageUrl } from "@/lib/image-cdn";

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
    avatar: "/api/assets/blog/testimonials/tobi.jpg",
    url: "https://x.com/tobi/status/1983892259230699921",
  },
  {
    quote: "Anarlog is worth a look.",
    author: "Anand Chowdhary",
    username: "AnandChowdhary",
    avatar: "/api/assets/blog/testimonials/anand.jpg",
    url: "https://x.com/AnandChowdhary/status/1997980479698723119",
  },
  {
    quote: "Anarlog is one of my favorite AI secret weapons.",
    author: "James Koshigoe",
    username: "JamesKoshigoe",
    avatar: "/api/assets/blog/testimonials/james-k.jpg",
    url: "https://x.com/JamesKoshigoe/status/2024676687980671195",
  },
  {
    quote: "Really liking Anarlog. Open access to my data and a GPL codebase!",
    author: "James LePage",
    username: "jameswlepage",
    avatar: "/api/assets/blog/testimonials/james-l.jpg",
    url: "https://x.com/jameswlepage/status/2042780872693166169",
  },
  {
    quote:
      "I love the flexibility that Anarlog gives me to integrate personal notes with AI summaries.",
    author: "Tom Yang",
    username: "tomyang11_",
    avatar: "/api/assets/blog/testimonials/tom.jpg",
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
        "border-color-subtle absolute rounded-xl border bg-white p-5 text-left transition-[transform,box-shadow,opacity] duration-500 ease-[cubic-bezier(0.22,1,0.36,1)] select-none [corner-shape:squircle] motion-reduce:transition-none sm:p-5",
        className,
      ])}
      style={style}
    >
      <figure className="flex h-full flex-col">
        <figcaption className="flex items-start justify-between gap-4">
          <div className="flex min-w-0 items-center gap-3">
            <img
              src={getResizedImageUrl(testimonial.avatar, {
                width: 48,
                height: 48,
              })}
              srcSet={getResizedImageSrcSet(testimonial.avatar, 48)}
              alt={`${testimonial.author} profile photo`}
              width={48}
              height={48}
              className="size-12 rounded-full object-cover shadow-sm"
              decoding="async"
              loading="lazy"
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

export function CredibilityLogoMarquee() {
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
                    "h-6 w-auto max-w-none object-contain opacity-65 grayscale transition-[filter,opacity] duration-200 hover:opacity-100 hover:grayscale-0",
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
            <Link
              to="/download/"
              className="inline-flex items-center gap-2 rounded-full bg-[#181613] px-5 py-3 text-sm font-medium text-white transition-colors hover:bg-[#4f4940]"
            >
              Start using for free
              <ArrowRight size={16} weight="bold" aria-hidden="true" />
            </Link>
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
