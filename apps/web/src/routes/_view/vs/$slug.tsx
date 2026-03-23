import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { allVs } from "content-collections";
import { useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { GitHubOpenSource } from "@/components/github-open-source";
import { SlashSeparator } from "@/components/slash-separator";
import {
  CoolStuffSection,
  CTASection,
  HowItWorksSection,
  MainFeaturesSection,
  TemplatesSection,
} from "@/routes/_view/index";

export const Route = createFileRoute("/_view/vs/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const doc = allVs.find((doc) => doc.slug === params.slug);
    if (!doc) {
      throw notFound();
    }

    return { doc };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.doc) {
      return { meta: [] };
    }

    const { doc } = loaderData;
    const metaTitle = `Char vs ${doc.name} - Privacy-First AI Notetaking`;

    return {
      meta: [
        { title: metaTitle },
        { name: "description", content: doc.metaDescription },
        { name: "robots", content: "noindex, nofollow" },
        { property: "og:title", content: metaTitle },
        { property: "og:description", content: doc.metaDescription },
        { property: "og:type", content: "website" },
        {
          property: "og:url",
          content: `https://char.com/vs/${doc.slug}`,
        },
        { name: "twitter:card", content: "summary" },
        { name: "twitter:title", content: metaTitle },
        { name: "twitter:description", content: doc.metaDescription },
      ],
    };
  },
});

function Component() {
  const { doc } = Route.useLoaderData();
  const [selectedFeature, setSelectedFeature] = useState(0);
  const featuresScrollRef = useRef<HTMLDivElement>(null);
  const heroInputRef = useRef<HTMLInputElement>(null);

  const scrollToFeature = (index: number) => {
    setSelectedFeature(index);
    if (featuresScrollRef.current) {
      const container = featuresScrollRef.current;
      const scrollLeft = container.offsetWidth * index;
      container.scrollTo({ left: scrollLeft, behavior: "smooth" });
    }
  };

  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection
          competitorIcon={doc.icon}
          competitorName={doc.name}
          headline={doc.headline}
          description={doc.description}
        />
        <SlashSeparator />
        <HowItWorksSection />
        <SlashSeparator />
        <CoolStuffSection />
        <SlashSeparator />
        <MainFeaturesSection
          featuresScrollRef={featuresScrollRef}
          selectedFeature={selectedFeature}
          setSelectedFeature={setSelectedFeature}
          scrollToFeature={scrollToFeature}
        />
        <SlashSeparator />
        <TemplatesSection />
        <SlashSeparator />
        <GitHubOpenSource />
        <SlashSeparator />
        <CTASection heroInputRef={heroInputRef} />
      </div>
    </div>
  );
}

function HeroSection({
  competitorIcon,
  competitorName,
  headline,
  description,
}: {
  competitorIcon: string;
  competitorName: string;
  headline: string;
  description: string;
}) {
  return (
    <div className="bg-linear-to-b from-stone-50/30 to-stone-100/30 px-6 py-12 lg:py-20">
      <header className="mx-auto max-w-4xl text-center">
        <div className="mb-12 flex flex-row items-center justify-center sm:gap-0">
          <div className="relative h-40 w-40 sm:hidden">
            <div className="absolute top-0 left-0 z-0 flex size-28 items-center justify-center rounded-4xl border border-neutral-100 bg-white opacity-50 shadow-2xl">
              <img
                src={competitorIcon}
                alt={competitorName}
                className="size-24 rounded-[28px] border border-neutral-100"
              />
            </div>
            <div className="absolute right-0 bottom-0 z-10 flex size-28 items-center justify-center rounded-4xl border border-neutral-100 bg-white shadow-2xl">
              <img
                src="/api/images/hyprnote/icon.png"
                alt="Char"
                className="size-24 rounded-[28px] border border-neutral-100"
              />
            </div>
          </div>
          <div className="hidden sm:flex sm:flex-row sm:items-center sm:gap-0">
            <div className="flex size-32 items-center justify-center rounded-[40px] border border-neutral-100 bg-transparent opacity-30 shadow-2xl">
              <img
                src={competitorIcon}
                alt={competitorName}
                className="size-28 rounded-4xl border border-neutral-100"
              />
            </div>
            <div className="pr-6 pl-5 text-3xl font-light text-neutral-400">
              vs
            </div>
            <div className="flex size-32 scale-110 items-center justify-center rounded-[40px] border border-neutral-100 bg-transparent shadow-2xl">
              <img
                src="/api/images/hyprnote/icon.png"
                alt="Char"
                className="size-28 rounded-4xl border border-neutral-100"
              />
            </div>
          </div>
        </div>

        <h1 className="mb-6 font-serif text-3xl tracking-tight text-stone-700 sm:text-4xl lg:text-5xl">
          {headline}
        </h1>
        <p className="mb-8 text-lg text-neutral-600 sm:text-xl">
          {description}
        </p>

        <div className="mt-8">
          <Link
            to="/download/"
            className={cn([
              "inline-block rounded-full px-8 py-3 text-base font-medium",
              "bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "transition-transform hover:scale-105 active:scale-95",
            ])}
          >
            Download Char for free
          </Link>
        </div>
      </header>
    </div>
  );
}
