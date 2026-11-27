import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@anlg/utils";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { PricingSection } from "@/components/home-page/pricing-section";
import { SiteFooter } from "@/components/site-footer";
import {
  ANARLOG_ROW,
  COMPARISON_ROWS,
  type ComparisonRow,
  COMPARISON_REVIEWED_ON,
} from "@/lib/competitors";
import { getCanonicalUrl } from "@/lib/seo";

const title = "Pricing · Anarlog";
const description =
  "Compare Anarlog Free, Pro, and Enterprise. Pro adds dictation, cloud features, and team collaboration for $14/person/month or $140/person/year.";

const reviewedOnLabel = new Date(COMPARISON_REVIEWED_ON).toLocaleDateString(
  "en-US",
  { month: "long", day: "numeric", year: "numeric", timeZone: "UTC" },
);

export const Route = createFileRoute("/pricing/")({
  component: PricingPage,
  head: () => ({
    meta: [
      { title },
      { name: "description", content: description },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: getCanonicalUrl("/pricing") },
      { name: "twitter:title", content: title },
      { name: "twitter:description", content: description },
      { name: "twitter:url", content: getCanonicalUrl("/pricing") },
    ],
    links: [{ rel: "canonical", href: getCanonicalUrl("/pricing") }],
  }),
});

function PricingPage() {
  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 pt-4 pb-8 md:px-8 md:pt-4 md:pb-12">
        <div className="min-w-0 text-center">
          <section className="pt-10 md:pt-12">
            <Link to="/" aria-label="Anarlog home" className="inline-flex">
              <AnarlogLogo className="h-8 w-auto md:h-9" />
            </Link>
          </section>

          <PricingSection headingLevel="h1" />

          <p className="text-color-muted mt-6 text-sm leading-6">
            YC founder? Check the{" "}
            <Link to="/yc/" className="text-color underline underline-offset-4">
              YC founder perk
            </Link>{" "}
            for eligibility and verification.
          </p>

          <section className="pt-12 pb-8 md:pt-16 md:pb-10">
            <h2 className="text-2xl leading-tight font-semibold tracking-tight text-[#756b5d] md:text-3xl">
              How we compare
            </h2>
            <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
              Compare the recording method, AI processing and stored meeting
              record separately. A bot-free app can still use hosted AI, and a
              free recording allowance can have separate AI or storage limits.
            </p>

            <div className="relative left-1/2 mt-8 w-screen max-w-[1100px] -translate-x-1/2 px-5 md:px-8">
              <div className="overflow-x-auto rounded-3xl border border-[#eadfce] [corner-shape:round]">
                <table className="w-full min-w-[760px] border-collapse text-left text-sm">
                  <thead>
                    <tr className="border-b border-[#eadfce]">
                      <Th sticky>Tool</Th>
                      <Th>Recording and processing</Th>
                      <Th>Free plan or trial</Th>
                      <Th>Example paid plan</Th>
                    </tr>
                  </thead>
                  <tbody>
                    <Row row={ANARLOG_ROW} highlight />
                    {COMPARISON_ROWS.map((row) => (
                      <Row key={row.name} row={row} />
                    ))}
                  </tbody>
                </table>
              </div>

              <p className="mt-4 text-left text-xs leading-5 text-[#756b5d]">
                Source review: {reviewedOnLabel}. Quoted prices are USD with the
                billing basis shown; annual rates require annual billing. Some
                plan prices are left unquoted where currency, availability or
                conflicting source details were unresolved. Follow each vendor's
                plans and recording guide for current conditions.
              </p>
              <p className="mt-2 text-left text-xs leading-5 text-[#756b5d]">
                Local speech recognition does not make summaries local. For
                setup, use the{" "}
                <Link
                  to="/guides/transcription/"
                  className="underline underline-offset-4"
                >
                  transcription guides
                </Link>{" "}
                and compare detailed allowances in the{" "}
                <Link
                  to="/blog/$slug/"
                  params={{ slug: "free-ai-notetakers" }}
                  className="underline underline-offset-4"
                >
                  free AI notetaker guide
                </Link>
                . Confirm required participant notice and permission before
                recording.
              </p>
            </div>
          </section>
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}

function Th({
  children,
  sticky = false,
}: {
  children: React.ReactNode;
  sticky?: boolean;
}) {
  return (
    <th
      scope="col"
      className={cn([
        "bg-[#fffaf0] px-4 py-3 text-xs font-semibold whitespace-nowrap text-[#756b5d]",
        sticky && "sticky left-0 z-10",
      ])}
    >
      {children}
    </th>
  );
}

function Row({ row, highlight }: { row: ComparisonRow; highlight?: boolean }) {
  const isInternal = row.url.startsWith("/");

  return (
    <tr
      className={cn([
        "border-b border-[#f0e7d8] last:border-b-0",
        highlight && "bg-[#fff8e6]",
      ])}
    >
      <td
        className={cn([
          "sticky left-0 z-10 px-4 py-3 whitespace-nowrap",
          highlight ? "bg-[#fff8e6]" : "bg-white",
        ])}
      >
        <div className="flex items-center gap-2.5">
          {row.icon ? (
            <img
              src={row.icon}
              alt=""
              aria-hidden="true"
              className="size-5 shrink-0 rounded-[5px] object-contain"
            />
          ) : (
            <span
              aria-hidden="true"
              className="flex size-5 shrink-0 items-center justify-center rounded-[5px] bg-[#f4efe6] text-[10px] font-semibold text-[#756b5d]"
            >
              {row.name.charAt(0)}
            </span>
          )}
          {isInternal ? (
            <span className="font-semibold text-[#181613]">{row.name}</span>
          ) : (
            <a
              href={row.url}
              target="_blank"
              rel="noopener noreferrer"
              className="text-[#4f4940] underline decoration-[#d9cdb8] underline-offset-4 hover:text-[#181613]"
            >
              {row.name}
            </a>
          )}
        </div>
      </td>
      <td className="text-color px-4 py-3 align-top">
        {row.capture}
        {row.captureSourceUrl ? (
          <a
            href={row.captureSourceUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="mt-2 block text-xs underline underline-offset-4"
          >
            Recording details
          </a>
        ) : null}
      </td>
      <td className="text-color px-4 py-3 align-top">{row.freeTier}</td>
      <td className="text-color px-4 py-3 align-top">{row.paidPlan}</td>
    </tr>
  );
}
