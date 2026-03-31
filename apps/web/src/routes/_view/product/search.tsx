import { Icon } from "@iconify-icon/react";
import { createFileRoute } from "@tanstack/react-router";
import { SearchIcon } from "lucide-react";

import { Typewriter } from "@hypr/ui/components/ui/typewriter";

import { CTASection } from "@/components/cta-section";
import { DownloadButton } from "@/components/download-button";
import { GithubStars } from "@/components/github-stars";

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
      { property: "og:title", content: "Search - Char" },
      {
        property: "og:description",
        content:
          "Search your entire meeting history in seconds. Find exactly what was said, when it was said, and who said it.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/product/search",
      },
    ],
  }),
});

function Component() {
  return (
    <div className="min-h-screen overflow-x-hidden md:px-8">
      <div className="mx-auto">
        <HeroSection />
        <HowItWorksSection />
        <UseCasesSection />
        <FlexibilitySection />
        <CTASection />
      </div>
    </div>
  );
}

const searchQueries = [
  "Q3 marketing strategy discussion",
  "client feedback on product demo",
  "budget planning for next quarter",
  "project timeline with Sarah",
  "brainstorming session notes",
];

function HeroSection() {
  return (
    <div>
      <div className="py-12 lg:py-20">
        <header className="text-left">
          <h1 className="text-color mb-6 font-mono text-2xl tracking-wide sm:text-5xl">
            Search your entire meeting history in seconds
          </h1>
          <p className="text-fg-muted text-lg sm:text-xl">
            Find exactly what was said, when it was said, and who said it.
          </p>
          <div className="mt-8 flex items-center gap-4">
            <DownloadButton />
            <GithubStars />
          </div>
        </header>
      </div>

      <div className="pb-8">
        <div
          className="border-border bg-surface-subtle overflow-hidden rounded-xl border bg-cover bg-center"
          style={{
            backgroundImage: "url(/api/images/texture/bg-stars.jpg)",
          }}
        >
          <div className="py-16">
            <div className="relative mx-auto flex max-w-2xl flex-col gap-3">
              <div className="flex items-center gap-3 rounded-full border border-stone-300 bg-white px-4 py-3 shadow-[0_4px_6px_-1px_rgba(255,255,255,0.3),0_2px_4px_-2px_rgba(255,255,255,0.3)]">
                <SearchIcon className="size-5 shrink-0 text-stone-400" />
                <div className="min-w-0 flex-1 overflow-hidden text-left">
                  <Typewriter
                    text={searchQueries}
                    speed={100}
                    deleteSpeed={30}
                    waitTime={2000}
                    className="block truncate text-base font-light text-stone-700 sm:text-lg"
                    cursorClassName="ml-1"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function HowItWorksSection() {
  return (
    <section>
      <div className="border-color-brand border-b text-left">
        <p className="text-fg-muted py-6 font-mono font-medium tracking-wide uppercase">
          How it works
        </p>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="border-color-brand border-r border-b p-8 md:border-b-0">
          <Icon icon="mdi:magnify" className="text-color mb-4 text-3xl" />
          <h3 className="text-color mb-2 font-mono text-xl">Quick search</h3>
          <p className="text-fg-muted">
            Type in the search bar, get instant semantic results, navigate with
            arrow keys.
          </p>
        </div>
        <div className="p-8">
          <Icon
            icon="mdi:filter-variant"
            className="text-color mb-4 text-3xl"
          />
          <h3 className="text-color mb-2 font-mono text-xl">Advanced search</h3>
          <p className="text-fg-muted">
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
      <div className="border-color-brand border-b text-left">
        <p className="text-fg-muted py-6 font-mono font-medium tracking-wide uppercase">
          Your meeting history becomes useful
        </p>
      </div>
      <div className="grid md:grid-cols-2">
        <div className="border-color-brand border-r border-b p-8">
          <Icon icon="mdi:phone" className="text-color mb-4 text-3xl" />
          <h3 className="text-color mb-2 font-mono text-xl">
            Before a client call
          </h3>
          <p className="text-fg-muted">
            Pull up everything discussed in previous meetings—pricing,
            commitments, concerns.
          </p>
        </div>
        <div className="border-color-brand border-b p-8">
          <Icon
            icon="mdi:chart-timeline-variant"
            className="text-color mb-4 text-3xl"
          />
          <h3 className="text-color mb-2 font-mono text-xl">
            During quarterly reviews
          </h3>
          <p className="text-fg-muted">
            Search all team syncs to see what blockers came up repeatedly.
          </p>
        </div>
        <div className="border-color-brand border-r p-8">
          <Icon icon="mdi:account-plus" className="text-color mb-4 text-3xl" />
          <h3 className="text-color mb-2 font-mono text-xl">
            When onboarding someone
          </h3>
          <p className="text-fg-muted">
            Find every decision and context discussion without creating a
            separate doc.
          </p>
        </div>
        <div className="p-8">
          <Icon
            icon="mdi:comment-question"
            className="text-color mb-4 text-3xl"
          />
          <h3 className="text-color mb-2 font-mono text-xl">
            Settling disagreements
          </h3>
          <p className="text-fg-muted">
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
    <section>
      <div className="border-color-brand border-b text-left">
        <p className="text-fg-muted py-6 font-mono font-medium tracking-wide uppercase">
          Beyond built-in search
        </p>
      </div>
      <div className="p-8">
        <h3 className="text-color mb-4 font-mono text-xl">
          You're not restricted to Char's built-in search
        </h3>
        <p className="text-fg-muted max-w-3xl leading-relaxed">
          Since every note is a .md file on your device, search them however you
          want. Use Spotlight. Or{" "}
          <code className="bg-surface-subtle border-border-subtle rounded border px-1.5 py-0.5 font-mono text-sm">
            grep
          </code>{" "}
          from terminal. Or your IDE's search. Or Obsidian's graph view. Your
          choice.
        </p>
      </div>
    </section>
  );
}
