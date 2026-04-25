import { createFileRoute } from "@tanstack/react-router";
import {
  ArrowRightIcon,
  CheckIcon,
  LockIcon,
  MicIcon,
  PencilIcon,
} from "lucide-react";

import {
  CHAR_SITE_URL,
  ROOT_DESCRIPTION,
  getOrganizationJsonLd,
  getSoftwareApplicationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

const features = [
  {
    icon: MicIcon,
    title: "Bot-free capture",
    description:
      "Record meetings without inviting another participant or changing how your calls work.",
  },
  {
    icon: PencilIcon,
    title: "Notes you own",
    description:
      "Keep transcripts, summaries, and markdown notes under your control from the start.",
  },
  {
    icon: LockIcon,
    title: "Private by default",
    description:
      "Use local transcription, your own AI keys, or managed cloud AI when your workflow needs it.",
  },
];

const promises = [
  "Local-first desktop app",
  "Markdown-friendly notes",
  "Bring your own AI keys",
  "Built for meetings, calls, and interviews",
];

export const Route = createFileRoute("/_view/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: CHAR_SITE_URL }],
    scripts: [
      {
        type: "application/ld+json",
        children: JSON.stringify(
          getStructuredDataGraph([
            getOrganizationJsonLd(),
            getSoftwareApplicationJsonLd({
              description: ROOT_DESCRIPTION,
              featureList: promises,
            }),
          ]),
        ),
      },
    ],
  }),
});

function Component() {
  return (
    <main className="mx-auto flex min-h-screen w-full max-w-7xl flex-col px-4 py-5 sm:px-6 lg:px-8">
      <header className="border-color-subtle flex items-center justify-between gap-4 border-b py-4">
        <a href="/" className="text-color flex items-center gap-3">
          <img src="/favicon.svg" alt="" className="size-9" />
          <span className="font-mono text-lg font-medium">Char</span>
        </a>
        <a
          href="mailto:team@char.com"
          className="rounded-full border border-neutral-200 bg-white px-4 py-2 font-mono text-sm text-neutral-700 transition hover:bg-neutral-50"
        >
          Contact
        </a>
      </header>

      <section className="grid flex-1 items-center gap-10 py-14 lg:grid-cols-[minmax(0,1.02fr)_minmax(360px,0.78fr)] lg:py-20">
        <div className="max-w-4xl">
          <p className="text-color-muted mb-5 font-mono text-sm">
            Private meeting notes for people who want control of their work.
          </p>
          <h1 className="text-color text-5xl leading-tight font-medium tracking-normal sm:text-6xl lg:text-7xl">
            Meeting notes you own.
          </h1>
          <p className="text-color-muted mt-6 max-w-2xl text-lg sm:text-xl">
            Char captures calls without a bot, turns them into useful notes, and
            keeps your meeting data portable from day one.
          </p>

          <div className="mt-9 flex flex-col gap-3 sm:flex-row">
            <a
              href="mailto:team@char.com?subject=Char%20early%20access"
              className="inline-flex items-center justify-center gap-2 rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-5 py-3 font-mono text-sm text-white shadow-sm transition hover:from-stone-700 hover:to-stone-600"
            >
              Request access
              <ArrowRightIcon className="size-4" aria-hidden="true" />
            </a>
            <a
              href="https://github.com/fastrepl/hyprnote"
              className="inline-flex items-center justify-center rounded-full border border-neutral-200 bg-white px-5 py-3 font-mono text-sm text-neutral-700 transition hover:bg-neutral-50"
            >
              View source
            </a>
          </div>
        </div>

        <div className="surface border-around relative overflow-hidden rounded-lg p-4 shadow-sm">
          <div className="border-color-subtle bg-page rounded-md border p-4">
            <div className="border-color-subtle mb-4 flex items-center justify-between border-b pb-3">
              <div>
                <p className="text-color-muted font-mono text-xs">Today</p>
                <h2 className="text-color text-lg">Product sync</h2>
              </div>
              <span className="text-color-muted rounded-full bg-white px-3 py-1 font-mono text-xs">
                Live
              </span>
            </div>
            <div className="space-y-3">
              <div className="rounded-md bg-white p-3">
                <p className="text-color-muted font-mono text-xs">Transcript</p>
                <p className="text-color mt-1 text-base">
                  Action items, decisions, and context stay attached to the
                  meeting.
                </p>
              </div>
              <div className="rounded-md bg-white p-3">
                <p className="text-color-muted font-mono text-xs">Summary</p>
                <p className="text-color mt-1 text-base">
                  Char drafts the recap while your original notes stay editable.
                </p>
              </div>
              <div className="rounded-md bg-white p-3">
                <p className="text-color-muted font-mono text-xs">Export</p>
                <p className="text-color mt-1 text-base">
                  Keep the result as files you can move, search, and reuse.
                </p>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="grid gap-4 pb-16 md:grid-cols-3">
        {features.map((feature) => (
          <article
            key={feature.title}
            className="surface border-around rounded-lg p-5"
          >
            <feature.icon className="text-color-muted mb-5 size-5" />
            <h2 className="text-color text-xl">{feature.title}</h2>
            <p className="text-color-muted mt-3 text-base">
              {feature.description}
            </p>
          </article>
        ))}
      </section>

      <section className="border-color-subtle mb-10 border-t py-6">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {promises.map((promise) => (
            <div key={promise} className="text-color flex items-center gap-2">
              <CheckIcon
                className="text-color-muted size-4"
                aria-hidden="true"
              />
              <span className="text-sm">{promise}</span>
            </div>
          ))}
        </div>
      </section>
    </main>
  );
}
