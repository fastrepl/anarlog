import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useRef } from "react";

import { cn } from "@hypr/utils";

import { SlashSeparator } from "@/components/slash-separator";
import { CTASection } from "@/routes/_view/index";

export const Route = createFileRoute("/_view/product/search")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Search - Char" },
      {
        name: "description",
        content:
          "Search your entire meeting history in seconds. Find exactly what was said, when it was said, and who said it.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  const heroInputRef = useRef<HTMLInputElement>(null);

  return (
    <main
      className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection />
        <SlashSeparator />
        <HowItWorksSection />
        <SlashSeparator />
        <UseCasesSection />
        <SlashSeparator />
        <FlexibilitySection />
        <SlashSeparator />
        <CTASection heroInputRef={heroInputRef} />
      </div>
    </main>
  );
}

function HeroSection() {
  return (
    <section className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="flex flex-col items-center gap-6 px-4 py-24 text-center">
        <div className="flex max-w-4xl flex-col gap-6">
          <h1 className="font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Search your entire meeting history in seconds
          </h1>
          <p className="mx-auto max-w-3xl text-lg text-neutral-600 sm:text-xl">
            Find exactly what was said, when it was said, and who said it.
          </p>
        </div>
        <div className="flex flex-col gap-4 pt-6 sm:flex-row">
          <Link
            to="/download/"
            className={cn([
              "rounded-full px-8 py-3 text-base font-medium",
              "bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
              "transition-all",
            ])}
          >
            Download for free
          </Link>
        </div>
      </div>
    </section>
  );
}

function HowItWorksSection() {
  return (
    <section className="bg-stone-50/30">
      <div className="p-8">
        <h2 className="mb-8 text-center font-serif text-3xl text-stone-700">
          How it works
        </h2>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="border-r border-neutral-100 p-8">
          <Icon icon="mdi:magnify" className="mb-4 text-3xl text-stone-700" />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Quick search
          </h3>
          <p className="text-neutral-600">
            Type in the search bar, get instant semantic results, navigate with
            arrow keys.
          </p>
        </div>
        <div className="p-8">
          <Icon
            icon="mdi:filter-variant"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Advanced search
          </h3>
          <p className="text-neutral-600">
            Filter by date, person, or organization. Use quotes for exact phrase
            matching.
          </p>
        </div>
      </div>
    </section>
  );
}

function UseCasesSection() {
  return (
    <section>
      <div className="p-8">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          Your meeting history becomes useful
        </h2>
        <p className="mx-auto max-w-2xl text-center text-lg text-neutral-600">
          With search, your meeting history becomes a knowledge base you
          actually use
        </p>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="border-r border-b border-neutral-100 p-8">
          <Icon icon="mdi:phone" className="mb-4 text-3xl text-stone-700" />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Before a client call
          </h3>
          <p className="text-neutral-600">
            Pull up everything discussed in previous meetings—pricing,
            commitments, concerns.
          </p>
        </div>
        <div className="border-b border-neutral-100 p-8">
          <Icon
            icon="mdi:chart-timeline-variant"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            During quarterly reviews
          </h3>
          <p className="text-neutral-600">
            Search all team syncs to see what blockers came up repeatedly.
          </p>
        </div>
        <div className="border-r border-neutral-100 p-8">
          <Icon
            icon="mdi:account-plus"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            When onboarding someone
          </h3>
          <p className="text-neutral-600">
            Find every decision and context discussion without creating a
            separate doc.
          </p>
        </div>
        <div className="p-8">
          <Icon
            icon="mdi:comment-question"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-2 font-serif text-xl text-stone-700">
            Settling disagreements
          </h3>
          <p className="text-neutral-600">
            "I'm pretty sure we decided on version A" — Find the exact
            conversation.
          </p>
        </div>
      </div>
    </section>
  );
}

function FlexibilitySection() {
  return (
    <section className="bg-stone-50/30">
      <div className="p-8">
        <h2 className="mb-4 text-center font-serif text-3xl text-stone-700">
          You're not restricted to Char's built-in search
        </h2>
        <p className="mx-auto max-w-3xl text-center text-lg text-neutral-600">
          Since every note is a .md file on your device, search them however you
          want. Use Spotlight. Or{" "}
          <code className="rounded bg-neutral-100 px-1.5 py-0.5 font-mono text-sm">
            grep
          </code>{" "}
          from terminal. Or your IDE's search. Or Obsidian's graph view. Your
          choice.
        </p>
      </div>
    </section>
  );
}
