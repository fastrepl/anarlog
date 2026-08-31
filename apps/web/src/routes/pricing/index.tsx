import { CheckCircle, XCircle } from "@phosphor-icons/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@anlg/utils";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { PricingSection } from "@/components/home-page/pricing-section";
import { SiteFooter } from "@/components/site-footer";
import {
  ANARLOG_ROW,
  COMPARISON_ROWS,
  type ComparisonRow,
  PRICING_VERIFIED_ON,
} from "@/lib/competitors";
import { getCanonicalUrl } from "@/lib/seo";

const title = "Pricing · Anarlog";
const description =
  "Anarlog is free for unlimited local transcription, with Pro at $15/month and Team at $20/person/month. Compare pricing, capture method, and data ownership against other AI notetakers.";

const verifiedOnLabel = new Date(PRICING_VERIFIED_ON).toLocaleDateString(
  "en-US",
  { month: "long", day: "numeric", year: "numeric" },
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

          <PricingSection />

          <section className="pt-12 pb-8 md:pt-16 md:pb-10">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
              How we compare
            </h2>
            <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
              Most AI notetakers send your meeting to their servers and pick the
              AI for you. The column that matters is where your data lives.
            </p>

            <div className="relative left-1/2 mt-8 w-screen max-w-[980px] -translate-x-1/2 px-5 md:px-8">
              <div className="overflow-x-auto rounded-3xl border border-[#eadfce] [corner-shape:squircle]">
                <table className="w-full min-w-[860px] border-collapse text-left text-sm">
                  <thead>
                    <tr className="border-b border-[#eadfce]">
                      <Th sticky>Tool</Th>
                      <Th>Paid from</Th>
                      <Th>Free tier</Th>
                      <Th center>Bot-free</Th>
                      <Th center>Local data</Th>
                      <Th center>Offline</Th>
                      <Th center>Local models</Th>
                      <Th center>Own keys</Th>
                      <Th center>Open source</Th>
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
                Bot-free means capture without adding a participant to the call.
                Local data means the meeting record is stored on your device by
                default. Offline means recording and transcription work with no
                connection.
              </p>
              <p className="mt-2 text-left text-xs leading-5 text-[#756b5d]">
                Competitor details verified {verifiedOnLabel} from each vendor's
                published material, using the lowest regularly available paid
                tier. Plans change — follow the links above before deciding.
                Anarlog pricing is always current.
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
  center = false,
  sticky = false,
}: {
  children: React.ReactNode;
  center?: boolean;
  sticky?: boolean;
}) {
  return (
    <th
      scope="col"
      className={cn([
        "bg-[#fffaf0] px-4 py-3 text-xs font-semibold whitespace-nowrap text-[#756b5d]",
        center && "text-center",
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
              rel="noopener noreferrer nofollow"
              className="text-[#4f4940] underline decoration-[#d9cdb8] underline-offset-4 hover:text-[#181613]"
            >
              {row.name}
            </a>
          )}
        </div>
      </td>
      <td
        className={cn([
          "px-4 py-3 whitespace-nowrap",
          highlight ? "font-semibold text-[#181613]" : "text-[#4f4940]",
        ])}
      >
        {row.paidFrom}
      </td>
      <td className="px-4 py-3 whitespace-nowrap text-[#4f4940]">
        {row.freeTier}
      </td>
      <Bool value={row.botFree} />
      <Bool value={row.localData} />
      <Bool value={row.offline} />
      <Bool value={row.localModels} />
      <Bool value={row.ownKeys} />
      <Bool value={row.openSource} />
    </tr>
  );
}

function Bool({ value }: { value: boolean }) {
  const Icon = value ? CheckCircle : XCircle;

  return (
    <td className="px-4 py-3">
      <div className="flex justify-center">
        <Icon
          className={cn([
            "size-4.5",
            value ? "text-emerald-600" : "text-red-500",
          ])}
          aria-label={value ? "Yes" : "No"}
        />
      </div>
    </td>
  );
}
