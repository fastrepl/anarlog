import { createFileRoute, redirect } from "@tanstack/react-router";
import { ArrowRight, ChevronDown, KeyRound, WifiOff } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { z } from "zod";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

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
          <section className="pt-24 pb-16 md:pt-32">
            <h1 className="font-hand max-w-3xl text-6xl leading-[0.98] font-semibold tracking-normal text-balance md:text-8xl">
              AI notepad for private meetings.
            </h1>
            <p className="mt-6 max-w-2xl text-xl leading-9 text-[#363029]">
              An open-source Granola alternative for private meetings. Record
              locally and choose where AI runs.
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

          <HowItWorksSection />

          <PrivacySection />

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
      <div className="flex items-center gap-3 rounded-2xl border border-neutral-200 bg-white py-2 pr-8 pl-2 shadow-lg md:w-full">
        <div className="flex size-9 shrink-0 items-center justify-center rounded-full bg-emerald-500/10">
          <GoogleMeetIcon className="size-5" />
        </div>
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-stone-800">
            1-1 w/ Janice
          </span>
          <span className="text-sm text-stone-400">3 participants</span>
        </div>
      </div>
    </div>
  );
}

function GoogleMeetIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      aria-hidden="true"
      className={className}
      fill="none"
    >
      <path
        d="M4 7.5A2.5 2.5 0 0 1 6.5 5h6.75L16 7.75v8.75A2.5 2.5 0 0 1 13.5 19h-7A2.5 2.5 0 0 1 4 16.5v-9Z"
        fill="#00AC47"
      />
      <path d="M4 7.5A2.5 2.5 0 0 1 6.5 5H9v5.5H4v-3Z" fill="#FFBA00" />
      <path d="M4 13.5h5V19H6.5A2.5 2.5 0 0 1 4 16.5v-3Z" fill="#EA4335" />
      <path d="M9 5h4.25L16 7.75v2.75H9V5Z" fill="#2684FC" />
      <path d="M9 13.5h7v3A2.5 2.5 0 0 1 13.5 19H9v-5.5Z" fill="#00832D" />
      <path
        d="M16 9.25 20.4 6A1 1 0 0 1 22 6.8v10.4a1 1 0 0 1-1.6.8L16 14.75v-5.5Z"
        fill="#00AC47"
      />
      <path
        d="M16 9.25 20.4 6A1 1 0 0 1 22 6.8v3.95L16 14.75v-5.5Z"
        fill="#2684FC"
      />
    </svg>
  );
}

function HowItWorksSection() {
  const [typedText1, setTypedText1] = useState("");
  const [typedText2, setTypedText2] = useState("");
  const [enhancedLines, setEnhancedLines] = useState(0);

  const text1 = "metrisc w/ john";
  const text2 = "stakehlder mtg";

  useEffect(() => {
    const runAnimation = () => {
      setTypedText1("");
      setTypedText2("");
      setEnhancedLines(0);

      let currentIndex1 = 0;
      setTimeout(() => {
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

                setTimeout(() => {
                  setEnhancedLines(1);
                  setTimeout(() => {
                    setEnhancedLines(2);
                    setTimeout(() => {
                      setEnhancedLines(3);
                      setTimeout(() => {
                        setEnhancedLines(4);
                        setTimeout(() => {
                          setEnhancedLines(5);
                          setTimeout(() => {
                            setEnhancedLines(6);
                            setTimeout(() => runAnimation(), 3000);
                          }, 800);
                        }, 800);
                      }, 800);
                    }, 800);
                  }, 800);
                }, 500);
              }
            }, 50);
          }
        }, 50);
      }, 500);
    };

    runAnimation();
  }, []);

  return (
    <section className="py-10">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          How it works
        </h2>
        <p className="mt-6 text-lg leading-8 text-[#4f4940]">
          Write rough notes during the meeting. Anarlog turns them into an
          editable summary afterward.
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
              <span className="font-semibold">While you take notes,</span>{" "}
              Anarlog records from your device. No bot joins the call.
            </p>
          </div>
          <div className="flex flex-1 items-center justify-center bg-stone-50/30 px-8 pb-0">
            <div className="w-full max-w-lg overflow-hidden rounded-t-xl border border-b-0 border-neutral-200 bg-white shadow-lg">
              <div className="flex items-center gap-2 border-b border-neutral-200 bg-neutral-50 px-4 py-3">
                <div className="flex gap-2">
                  <div className="h-3 w-3 rounded-full bg-red-400"></div>
                  <div className="h-3 w-3 rounded-full bg-yellow-400"></div>
                  <div className="h-3 w-3 rounded-full bg-green-400"></div>
                </div>
                <div className="ml-auto">
                  <DancingSticks amplitude={1} height={12} color="#a3a3a3" />
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
              <span className="font-semibold">After the meeting is over,</span>{" "}
              your rough notes become a summary you can edit and keep.
            </p>
          </div>
          <div className="flex flex-1 items-start justify-center bg-stone-50/30 px-8 pb-0">
            <div className="w-full max-w-lg overflow-hidden rounded-t-xl border border-b-0 border-neutral-200 bg-white shadow-lg">
              <div className="flex items-center gap-2 border-b border-neutral-200 bg-neutral-50 px-4 py-3">
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
              <span className="font-semibold">While you take notes,</span>{" "}
              Anarlog records from your device. No bot joins the call.
            </p>
          </div>
          <div className="relative overflow-clip bg-stone-50/30 px-6 pb-0">
            <div
              className="pointer-events-none absolute right-0 bottom-0 left-0 z-10 h-20 bg-linear-to-t from-white to-transparent"
              aria-hidden="true"
            />
            <div className="overflow-hidden rounded-t-lg border border-b-0 border-neutral-200 bg-white shadow-lg">
              <div className="flex items-center gap-1.5 border-b border-neutral-200 bg-neutral-50 px-3 py-2">
                <div className="flex gap-1.5">
                  <div className="h-2 w-2 rounded-full bg-red-400"></div>
                  <div className="h-2 w-2 rounded-full bg-yellow-400"></div>
                  <div className="h-2 w-2 rounded-full bg-green-400"></div>
                </div>
                <div className="ml-auto">
                  <DancingSticks amplitude={1} height={10} color="#a3a3a3" />
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
              <span className="font-semibold">After the meeting is over,</span>{" "}
              your rough notes become a summary you can edit and keep.
            </p>
          </div>
          <div className="overflow-clip bg-stone-50/30 px-6 pb-0">
            <div className="overflow-hidden rounded-t-lg border border-b-0 border-neutral-200 bg-white shadow-lg">
              <div className="flex items-center gap-1.5 border-b border-neutral-200 bg-neutral-50 px-3 py-2">
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
        href="https://v2.char.com"
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
