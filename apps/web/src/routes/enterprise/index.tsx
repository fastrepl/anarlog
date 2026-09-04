import { BellRinging, CheckCircle, HardDrives } from "@phosphor-icons/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@anlg/utils";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { BookFounderCall } from "@/components/book-founder-call";
import { EnterpriseCtaLink } from "@/components/enterprise-cta-link";
import {
  LocalFilesVisual,
  MeetingCaptureVisual,
} from "@/components/home-page/privacy-section";
import { PilotPath } from "@/components/pilot-path";
import { SecurityReviewList } from "@/components/security-review-list";
import { SiteFooter } from "@/components/site-footer";
import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";
import { ENTERPRISE_EVENTS } from "@/lib/enterprise";
import { getCanonicalUrl } from "@/lib/seo";
import { proofStatus, shipsToday, shipsWithPartners } from "@/lib/trust-center";

const title = "Enterprise · Anarlog";
const description =
  "Anarlog for teams: local-first, bot-free meeting notes with end-to-end encrypted sync. Encryption, retention, training, and subprocessors — written so IT, security, and legal can review without a founder call.";

export const Route = createFileRoute("/enterprise/")({
  component: EnterprisePage,
  head: () => ({
    meta: [
      { title },
      { name: "description", content: description },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: getCanonicalUrl("/enterprise") },
      { name: "twitter:title", content: title },
      { name: "twitter:description", content: description },
      { name: "twitter:url", content: getCanonicalUrl("/enterprise") },
    ],
    links: [{ rel: "canonical", href: getCanonicalUrl("/enterprise") }],
  }),
});

const pillarRows = [
  [
    {
      title: "Private by architecture",
      body: "Notes live in local SQLite, and cloud sync is end-to-end encrypted — our servers only ever hold ciphertext.",
      Visual: LocalFilesVisual,
    },
    {
      title: "No bots in your meetings",
      body: "Anarlog listens locally. Nothing joins your calls, and nothing appears in participant lists.",
      Visual: MeetingCaptureVisual,
    },
    {
      title: "Consent on your terms",
      body: "Recording disclosure and consent defaults set once, org-wide — every meeting meets the same bar.",
      Visual: ConsentNoticeVisual,
    },
  ],
  [
    {
      title: "Admin without surveillance",
      body: "Members, roles, seats, and org-wide policies built on metadata — never on anyone's notes.",
      Visual: WorkspaceAdminVisual,
    },
    {
      title: "Self-host the whole stack",
      body: "Run the Anarlog server on infrastructure you control for regulated environments.",
      Visual: SelfHostVisual,
    },
  ],
];

function EnterprisePage() {
  const { track } = useAnalytics();

  useMountEffect(() => {
    track(ENTERPRISE_EVENTS.pageViewed, { page: "enterprise" });
  });

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 pt-4 pb-8 md:px-8 md:pt-4 md:pb-12">
        <div className="min-w-0 text-center">
          <section className="pt-10 pb-4 md:pt-12 md:pb-6">
            <Link to="/" aria-label="Anarlog home" className="inline-flex">
              <AnarlogLogo className="h-8 w-auto md:h-9" />
            </Link>
            <h1 className="font-hand mt-12 text-4xl leading-none font-semibold text-[#181613] md:mt-16 md:text-5xl">
              Meeting memory your company owns
            </h1>
            <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
              Bring Anarlog to your team without handing conversations to
              another cloud. Notes stay on employee machines, Cloud Sync is
              end-to-end encrypted, and no bot joins the call. This page is
              written so you can forward it to IT, security, and legal.
            </p>
            <div className="mt-8 flex flex-col items-center gap-3">
              <BookFounderCall location="hero" page="enterprise" />
              <EnterpriseCtaLink
                to="/security/"
                cta="security"
                location="hero"
                page="enterprise"
              >
                Read the security page
              </EnterpriseCtaLink>
            </div>
            <p className="mt-3 text-xs text-[#756b5d]">
              30 minutes, directly with the founder. No SDR queue.
            </p>
          </section>

          <section className="pt-12 pb-4 md:pt-16 md:pb-6">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              Why teams pick Anarlog
            </h2>
            <div className="relative left-1/2 mt-6 w-screen max-w-[1120px] -translate-x-1/2">
              <div className="flex flex-col gap-4 md:gap-8">
                {pillarRows.map((row) => (
                  <div
                    key={row[0].title}
                    className={cn([
                      "grid gap-4 md:flex md:items-start md:gap-0",
                      row.length === 3
                        ? "md:justify-between"
                        : "md:justify-evenly",
                    ])}
                  >
                    {row.map((pillar) => (
                      <div
                        key={pillar.title}
                        className="flex flex-col px-6 py-3 text-center md:w-[31%] md:p-4"
                      >
                        <pillar.Visual />
                        <h3 className="mt-5 text-base font-medium text-[#4f4940] md:mt-7">
                          {pillar.title}
                        </h3>
                        <p className="mx-auto mt-1 max-w-[17rem] text-sm leading-6 text-[#4f4940]">
                          {pillar.body}
                        </p>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          </section>

          <section className="pt-12 pb-4 md:pt-16 md:pb-6">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              For IT, security, and legal
            </h2>
            <p className="mx-auto mt-5 max-w-2xl text-base leading-7 text-[#4f4940]">
              The answers below are the same facts we use on questionnaires. The{" "}
              <EnterpriseCtaLink
                to="/security/"
                cta="security"
                location="security_review"
                page="enterprise"
                className="text-base text-[#4f4940]"
              >
                security page
              </EnterpriseCtaLink>{" "}
              has architecture, processors, retention, and the procurement
              packet.
            </p>
            <SecurityReviewList />
            <div className="mt-8 flex flex-col gap-3 text-left text-sm leading-6 text-[#4f4940] md:flex-row md:gap-8">
              <div className="flex-1">
                <h3 className="font-medium text-[#181613]">Ships today</h3>
                <ul className="mt-2 list-disc space-y-1.5 pl-5">
                  {shipsToday.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
              <div className="flex-1">
                <h3 className="font-medium text-[#181613]">
                  With early partners
                </h3>
                <ul className="mt-2 list-disc space-y-1.5 pl-5">
                  {shipsWithPartners.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
            </div>
          </section>

          <section className="pt-12 pb-4 md:pt-14 md:pb-6">
            <div
              className="flex items-center justify-center pb-4 select-none"
              aria-hidden="true"
            >
              <video
                className="h-24 w-auto md:h-32"
                src="/videos/partner-handshake.webm"
                autoPlay
                loop
                muted
                playsInline
                ref={(el) => {
                  if (
                    el &&
                    window.matchMedia("(prefers-reduced-motion: reduce)")
                      .matches
                  ) {
                    el.pause();
                  }
                }}
              />
            </div>
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              How an evaluation works
            </h2>
            <p className="mx-auto mt-5 max-w-2xl text-base leading-7 text-[#4f4940]">
              {proofStatus.body}
            </p>
            <PilotPath />
          </section>

          <section className="pt-8 pb-20 md:pt-10 md:pb-24">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              Talk to us
            </h2>
            <p className="mx-auto mt-5 max-w-lg text-base leading-7 text-[#4f4940]">
              Tell us about the team, the data boundary you need, and who on
              security has to sign off. We'll show what works today and what
              lands next.
            </p>
            <div className="mt-8 flex flex-col items-center gap-4">
              <BookFounderCall location="talk" page="enterprise" />
              <EnterpriseCtaLink
                to="/pricing/"
                cta="pricing"
                location="talk"
                page="enterprise"
              >
                Compare plans and pricing
              </EnterpriseCtaLink>
            </div>
          </section>
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}

function ConsentNoticeVisual() {
  return (
    <div className="flex h-20 items-center justify-center select-none md:h-28 md:w-full">
      <div className="flex w-full max-w-[260px] items-center gap-3 rounded-2xl border border-neutral-200 bg-white py-2 pr-3 pl-4 text-left shadow-[0_3px_10px_rgba(24,22,19,0.04)]">
        <BellRinging size={28} className="text-stone-700" aria-hidden="true" />
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-stone-800">
            Consent notice sent
          </span>
          <span className="text-sm text-stone-400">org-wide policy</span>
        </div>
        <CheckCircle
          size={20}
          weight="fill"
          className="ml-auto text-emerald-500"
          aria-hidden="true"
        />
      </div>
    </div>
  );
}

function WorkspaceAdminVisual() {
  return (
    <div className="flex h-20 items-center justify-center select-none md:h-28 md:w-full">
      <div className="flex w-full max-w-[260px] items-center gap-3 rounded-2xl border border-neutral-200 bg-white py-2 pr-3 pl-4 text-left shadow-[0_3px_10px_rgba(24,22,19,0.04)]">
        <div className="flex -space-x-2.5" aria-hidden="true">
          {["S", "B", "A"].map((initial) => (
            <span
              key={initial}
              className="flex h-7 w-7 items-center justify-center rounded-full border-2 border-white bg-[#eadfce] text-xs font-semibold text-[#756b5d] [corner-shape:round]"
            >
              {initial}
            </span>
          ))}
        </div>
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-stone-800">
            Design team
          </span>
          <span className="text-sm text-stone-400">12 seats</span>
        </div>
      </div>
    </div>
  );
}

function SelfHostVisual() {
  return (
    <div className="flex h-20 items-center justify-center select-none md:h-28 md:w-full">
      <div className="flex w-full max-w-[260px] items-center gap-3 rounded-2xl border border-neutral-200 bg-white py-2 pr-3 pl-4 text-left shadow-[0_3px_10px_rgba(24,22,19,0.04)]">
        <HardDrives size={28} className="text-stone-700" aria-hidden="true" />
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-stone-800">
            notes.acme.internal
          </span>
          <span className="text-sm text-stone-400">your infrastructure</span>
        </div>
        <span
          className="ml-auto h-2.5 w-2.5 rounded-full bg-emerald-500 [corner-shape:round]"
          aria-hidden="true"
        />
      </div>
    </div>
  );
}
