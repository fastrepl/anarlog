import { createFileRoute, Link } from "@tanstack/react-router";

import {
  CHAR_SITE_URL,
  ROOT_DESCRIPTION,
  getOrganizationJsonLd,
  getSoftwareApplicationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

const manifestoLetter = [
  "To the people who still take notes,",
  "We believe in the power of notetaking, not notetakers. A note-taker is passive. A notepad is something you use. You are present, you are engaged, and the tool works alongside you while the room is still alive.",
  "Most AI tools ask you to move your memory into their ecosystem, their models, and their rules. We think meeting notes should move in the other direction: back to files, back to your disk, and back to software you can run fully offline.",
  "Files endure. Interfaces change. Your notes should survive us. AI should be available through on-device models or your own keys, not through a service you cannot inspect.",
  "Anarlog is our attempt to build that kind of meeting notepad.",
  "John Jeong, Yujong Lee",
];

const featureList = [
  "Bot-free meeting capture",
  "Fully offline notes",
  "On-device or bring-your-own-key AI",
  "File-based storage",
  "Open source foundations",
];

const principles = [
  {
    title: "Stay in the meeting",
    description:
      "Anarlog is for people who want help while they are thinking, not a bot that replaces attention after the fact.",
  },
  {
    title: "Keep notes as files",
    description:
      "The record should live somewhere you can inspect, move, back up, and keep after any interface changes.",
  },
  {
    title: "Use AI on your terms",
    description:
      "Run on-device models or bring your own key. Those are the supported AI paths.",
  },
  {
    title: "Trust through transparency",
    description:
      "Open source makes the system inspectable. Meeting memory is too important to hide behind opaque behavior.",
  },
];

export const Route = createFileRoute("/")({
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
              featureList,
            }),
          ]),
        ),
      },
    ],
  }),
});

function Component() {
  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 py-8 md:px-8 md:py-12">
        <div className="min-w-0">
          <header className="flex items-center justify-between gap-6">
            <Link to="/" aria-label="Anarlog home">
              <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
            </Link>
          </header>

          <section className="pt-24 pb-16 md:pt-32">
            <h1 className="font-hand max-w-3xl text-6xl leading-[0.98] font-semibold tracking-normal text-balance md:text-8xl">
              AI notepad for private meetings.
            </h1>
            <p className="mt-6 max-w-2xl text-xl leading-9 text-[#363029]">
              Anarlog is an open-source alternative to Granola AI for people who
              care about privacy.
            </p>
            <div className="mt-8 flex flex-wrap gap-x-5 gap-y-3 text-sm">
              <a
                href="https://github.com/fastrepl/anarlog/releases/latest"
                className="inline-flex rounded-full bg-[#181613] px-5 py-3 font-medium text-white transition-colors hover:bg-[#4f4940]"
              >
                Download app
              </a>
            </div>
          </section>

          <section className="py-10">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Why Anarlog exists
            </h2>
            <ul className="mt-6 grid gap-8">
              {principles.map((principle) => (
                <li
                  key={principle.title}
                  className="grid gap-3 md:grid-cols-[13rem_1fr]"
                >
                  <p className="font-medium">{principle.title}</p>
                  <p className="leading-7 text-[#4f4940]">
                    {principle.description}
                  </p>
                </li>
              ))}
            </ul>
          </section>

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
                <p className="font-crisp-serif mt-5 text-base leading-none font-normal text-[#4f4940]">
                  Fastrepl, Inc.
                </p>
              </div>
            </div>
          </section>
        </div>
      </div>

      <footer className="mx-auto grid w-full max-w-[700px] gap-5 px-5 py-8 text-sm text-[#4f4940] md:grid-cols-[1fr_auto_1fr] md:items-center md:px-8">
        <Link
          to="/"
          aria-label="Anarlog home"
          className="md:justify-self-start"
        >
          <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
        </Link>
        <p className="text-xs text-[#756b5d] md:justify-self-center">
          Fastrepl © 2026
        </p>
        <nav className="flex flex-wrap gap-x-5 gap-y-2 md:justify-self-end">
          <a
            href="https://github.com/fastrepl/anarlog"
            className="hover:text-[#181613]"
          >
            GitHub
          </a>
          <a
            href="mailto:founders@fastrepl.com"
            className="hover:text-[#181613]"
          >
            Contact
          </a>
        </nav>
      </footer>
    </main>
  );
}
