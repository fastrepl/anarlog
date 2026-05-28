import { createFileRoute, Link, redirect } from "@tanstack/react-router";
import { ArrowRight, ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { z } from "zod";

import { SiteFooter } from "@/components/site-footer";
import { desktopSchemeSchema } from "@/functions/desktop-flow";
import { getGitHubStats } from "@/functions/github";
import {
  ANARLOG_SITE_URL,
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

function AnnouncementBanner() {
  return (
    <div className="flex justify-center px-5 pt-6 md:pt-8">
      <a
        href="https://v2.char.com"
        className="border-color-subtle text-color group inline-flex max-w-full items-center justify-center gap-2 rounded-full border bg-white px-4 py-2 text-center text-sm font-medium shadow-sm transition-colors hover:bg-neutral-50 md:px-5"
        aria-label="Visit Char v2"
      >
        <span className="min-w-0">We're working on something cool</span>
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
          className="absolute top-[calc(100%+0.5rem)] left-0 z-10 w-80 max-w-[calc(100vw-2.5rem)] rounded-2xl border border-[#d8d0c5] bg-white p-2 shadow-[0_14px_40px_rgba(24,22,19,0.12)]"
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
