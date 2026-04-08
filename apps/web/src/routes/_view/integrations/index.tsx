import { createFileRoute } from "@tanstack/react-router";
import { allIntegrations } from "content-collections";

import { AcquisitionLinkGrid } from "@/components/acquisition-link-grid";
import {
  CHAR_SITE_URL,
  getBreadcrumbListJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

const PLATFORM_ORDER = ["zoom", "google-meet", "teams", "webex"] as const;
const GUIDE_ORDER = [
  "notetaker",
  "transcription",
  "meeting-assistant",
] as const;

const PLATFORM_DESCRIPTIONS: Record<(typeof PLATFORM_ORDER)[number], string> = {
  zoom: "Zoom guides for bot-free meeting notes, transcription, and AI meeting assistance.",
  "google-meet":
    "Google Meet guides for private note capture, local transcription, and no-bot workflows.",
  teams:
    "Microsoft Teams guides for privacy-first notes, transcription, and meeting assistance.",
  webex:
    "Webex guides for searchable meeting notes, transcripts, and AI support without meeting bots.",
};

const GUIDE_LABELS: Record<(typeof GUIDE_ORDER)[number], string> = {
  notetaker: "AI notetaker",
  transcription: "Transcription",
  "meeting-assistant": "Meeting assistant",
};

const groupedIntegrations = PLATFORM_ORDER.map((category) => {
  const items = allIntegrations
    .filter((item) => item.category === category)
    .sort(
      (a, b) =>
        GUIDE_ORDER.indexOf(a.slug as (typeof GUIDE_ORDER)[number]) -
        GUIDE_ORDER.indexOf(b.slug as (typeof GUIDE_ORDER)[number]),
    );

  return {
    category,
    platform: items[0]?.platform ?? category,
    description: PLATFORM_DESCRIPTIONS[category],
    items,
  };
}).filter((group) => group.items.length > 0);

export const Route = createFileRoute("/_view/integrations/")({
  component: Component,
  head: () => {
    const url = `${CHAR_SITE_URL}/integrations`;
    const description =
      "Platform guides for Zoom, Google Meet, Microsoft Teams, and Webex. See how Char handles notetaking, transcription, and bot-free meeting assistance on each.";

    return {
      links: [{ rel: "canonical", href: url }],
      scripts: [
        {
          type: "application/ld+json",
          children: JSON.stringify(
            getStructuredDataGraph([
              {
                "@type": "CollectionPage",
                name: "Char integrations",
                url,
                description,
              },
              getBreadcrumbListJsonLd([
                { name: "Home", item: CHAR_SITE_URL },
                { name: "Integrations", item: url },
              ]),
            ]),
          ),
        },
      ],
      meta: [
        { title: "Integrations - Char" },
        { name: "description", content: description },
        { property: "og:title", content: "Integrations - Char" },
        { property: "og:description", content: description },
        { property: "og:type", content: "website" },
        { property: "og:url", content: url },
      ],
    };
  },
});

function Component() {
  return (
    <main className="min-h-screen px-4 py-16 md:px-8">
      <div className="mx-auto max-w-6xl">
        <header className="border-b border-neutral-200 pb-10 text-left">
          <div className="mb-4 font-mono text-xs tracking-[0.18em] text-stone-500 uppercase">
            Integrations
          </div>
          <h1 className="text-fg mb-4 font-mono text-4xl tracking-tight sm:text-5xl">
            Char for the meeting platforms you already use
          </h1>
          <p className="text-fg-muted max-w-3xl text-lg leading-8">
            Browse dedicated landing pages for Zoom, Google Meet, Microsoft
            Teams, and Webex. Each guide covers notetaking, transcription, or
            meeting-assistant workflows without the usual bot-in-the-call setup.
          </p>
        </header>

        <div className="flex flex-col gap-12 py-12">
          {groupedIntegrations.map((group) => (
            <section
              key={group.category}
              className="border-b border-neutral-200 pb-12 last:border-b-0 last:pb-0"
            >
              <div className="mb-6 max-w-3xl text-left">
                <div className="mb-2 font-mono text-xs tracking-[0.18em] text-stone-500 uppercase">
                  {group.platform}
                </div>
                <h2 className="text-fg mb-3 font-mono text-2xl tracking-tight sm:text-3xl">
                  {group.platform}
                </h2>
                <p className="text-fg-muted text-base leading-7">
                  {group.description}
                </p>
              </div>

              <div className="grid gap-4 md:grid-cols-3">
                {group.items.map((item) => (
                  <a
                    key={`${item.category}-${item.slug}`}
                    href={`/integrations/${item.category}/${item.slug}`}
                    className="group rounded-2xl border border-neutral-200 bg-white p-5 text-left transition-colors hover:border-stone-300 hover:bg-stone-50"
                  >
                    <div className="mb-3 font-mono text-[11px] tracking-[0.18em] text-stone-500 uppercase">
                      {GUIDE_LABELS[item.slug as (typeof GUIDE_ORDER)[number]]}
                    </div>
                    <h3 className="text-fg mb-2 font-mono text-lg">
                      {item.platform}{" "}
                      {GUIDE_LABELS[item.slug as (typeof GUIDE_ORDER)[number]]}
                    </h3>
                    <p className="text-fg-muted text-sm leading-6">
                      {item.metaDescription}
                    </p>
                    <div className="mt-4 text-sm font-medium text-stone-700 transition-colors group-hover:text-stone-950">
                      Open guide
                    </div>
                  </a>
                ))}
              </div>
            </section>
          ))}
        </div>

        <AcquisitionLinkGrid
          title="Next evaluation paths"
          description="If you are comparing options rather than browsing by platform, these are the fastest routes into the rest of the site."
          items={[
            {
              eyebrow: "Solutions",
              title: "Browse team workflows",
              description:
                "See how Char is positioned for sales, research, legal, coaching, and other conversation-heavy teams.",
              href: "/solutions/",
            },
            {
              eyebrow: "Pricing",
              title: "Compare Free, Lite, and Pro",
              description:
                "See which plan keeps work local, which one adds managed cloud AI, and what Pro unlocks.",
              href: "/pricing",
            },
            {
              eyebrow: "Comparisons",
              title: "Compare Char vs Otter",
              description:
                "Jump into one of the most common alternative pages for direct workflow and privacy tradeoffs.",
              href: "/vs/otter",
            },
          ]}
        />
      </div>
    </main>
  );
}
