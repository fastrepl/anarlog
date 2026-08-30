import { createFileRoute, Link } from "@tanstack/react-router";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { BookFounderCall } from "@/components/book-founder-call";
import { EnterpriseCtaLink } from "@/components/enterprise-cta-link";
import { PilotPath } from "@/components/pilot-path";
import { SecurityReviewList } from "@/components/security-review-list";
import { SiteFooter } from "@/components/site-footer";
import { useAnalytics } from "@/hooks/use-posthog";
import { useMountEffect } from "@/hooks/useMountEffect";
import {
  ENTERPRISE_EVENTS,
  PROCUREMENT_EMAIL,
  SECURITY_ADVISORY_URL,
  SECURITY_REPORT_EMAIL,
} from "@/lib/enterprise";
import { getCanonicalUrl } from "@/lib/seo";
import {
  architectureLayers,
  certificationStatus,
  contractualDocs,
  proofStatus,
  retentionRows,
  shipsToday,
  shipsWithPartners,
  subprocessors,
  TRUST_CENTER_UPDATED_ON,
} from "@/lib/trust-center";

const title = "Security · Anarlog";
const description =
  "How Anarlog handles encryption, data location, retention, training, subprocessors, and incident reporting — the packet for a first enterprise security review.";

export const Route = createFileRoute("/security")({
  component: SecurityPage,
  head: () => ({
    meta: [
      { title },
      { name: "description", content: description },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: getCanonicalUrl("/security") },
      { name: "twitter:title", content: title },
      { name: "twitter:description", content: description },
      { name: "twitter:url", content: getCanonicalUrl("/security") },
    ],
    links: [{ rel: "canonical", href: getCanonicalUrl("/security") }],
  }),
});

function SecurityPage() {
  const { track } = useAnalytics();

  useMountEffect(() => {
    track(ENTERPRISE_EVENTS.securityPageViewed, { page: "security" });
  });

  const updatedOn = new Date(
    `${TRUST_CENTER_UPDATED_ON}T00:00:00Z`,
  ).toLocaleDateString("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  });

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 pt-4 pb-8 md:px-8 md:pt-4 md:pb-12">
        <div className="min-w-0">
          <section className="pt-10 pb-4 text-center md:pt-12 md:pb-6">
            <Link to="/" aria-label="Anarlog home" className="inline-flex">
              <AnarlogLogo className="h-8 w-auto md:h-9" />
            </Link>
            <h1 className="font-hand mt-12 text-4xl leading-none font-semibold text-[#181613] md:mt-16 md:text-5xl">
              Security
            </h1>
            <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
              What a first security review needs: how Anarlog stores meeting
              data, who can see it, how long it is kept, and which documents we
              can send today. Fastrepl does not claim SOC 2, ISO 27001, or HIPAA
              here.
            </p>
            <p className="mt-3 text-xs text-[#756b5d]">
              Last updated {updatedOn}
            </p>
          </section>

          <section className="pt-10 pb-4 md:pt-12">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              Architecture
            </h2>
            <p className="mt-5 text-base leading-7 text-[#4f4940]">
              Local-first by default. Cloud features are optional and named.
              Nothing in the default product joins a meeting as a participant.
            </p>
            <ol className="mt-8 flex flex-col gap-4">
              {architectureLayers.map((layer, index) => (
                <li
                  key={layer.title}
                  className="rounded-2xl border border-[#eadfce] bg-[#fffaf0] px-5 py-4"
                >
                  <p className="text-xs tracking-[0.04em] text-[#756b5d]">
                    {String(index + 1).padStart(2, "0")}
                  </p>
                  <h3 className="mt-1 text-base font-medium text-[#181613]">
                    {layer.title}
                  </h3>
                  <p className="mt-1 text-sm leading-6 text-[#4f4940]">
                    {layer.body}
                  </p>
                </li>
              ))}
            </ol>
          </section>

          <section className="pt-10 pb-4 md:pt-12">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              Security review answers
            </h2>
            <SecurityReviewList detail />
          </section>

          <section className="pt-10 pb-4 md:pt-12">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              Subprocessors
            </h2>
            <p className="mt-5 text-base leading-7 text-[#4f4940]">
              Grounded in the{" "}
              <EnterpriseCtaLink
                to="/privacy/"
                cta="privacy"
                location="packet"
                page="security"
                className="text-base text-[#4f4940]"
              >
                privacy policy
              </EnterpriseCtaLink>
              . A processor receives data only when the matching feature is
              enabled.
            </p>
            <div className="mt-6 overflow-x-auto">
              <table className="w-full min-w-[36rem] border-collapse text-left text-sm">
                <thead>
                  <tr className="border-b border-[#eadfce] text-[#756b5d]">
                    <th className="py-2 pr-4 font-medium">Processor</th>
                    <th className="py-2 pr-4 font-medium">Purpose</th>
                    <th className="py-2 font-medium">What they can receive</th>
                  </tr>
                </thead>
                <tbody>
                  {subprocessors.map((row) => (
                    <tr key={row.name} className="border-b border-[#eadfce]">
                      <td className="py-3 pr-4 align-top font-medium text-[#181613]">
                        {row.name}
                      </td>
                      <td className="py-3 pr-4 align-top text-[#4f4940]">
                        {row.purpose}
                      </td>
                      <td className="py-3 align-top text-[#4f4940]">
                        {row.receives}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <section className="pt-10 pb-4 md:pt-12">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              Retention
            </h2>
            <div className="mt-6 overflow-x-auto">
              <table className="w-full min-w-[28rem] border-collapse text-left text-sm">
                <thead>
                  <tr className="border-b border-[#eadfce] text-[#756b5d]">
                    <th className="py-2 pr-4 font-medium">Data</th>
                    <th className="py-2 font-medium">Kept until</th>
                  </tr>
                </thead>
                <tbody>
                  {retentionRows.map((row) => (
                    <tr key={row.item} className="border-b border-[#eadfce]">
                      <td className="py-3 pr-4 align-top font-medium text-[#181613]">
                        {row.item}
                      </td>
                      <td className="py-3 align-top text-[#4f4940]">
                        {row.retention}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <section className="pt-10 pb-4 md:pt-12">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              Certifications and contracts
            </h2>
            <p className="mt-5 text-base leading-7 text-[#4f4940]">
              {certificationStatus.planned}{" "}
              {certificationStatus.hostedTrustCenter}
            </p>
            <ul className="mt-6 divide-y divide-[#eadfce] text-sm">
              {contractualDocs.map((doc) => (
                <li
                  key={doc.label}
                  className="flex flex-col gap-1 py-3 sm:flex-row sm:items-baseline sm:justify-between sm:gap-6"
                >
                  {doc.href.startsWith("/") ? (
                    <EnterpriseCtaLink
                      to={doc.href as "/privacy/" | "/terms/"}
                      cta={doc.href.includes("privacy") ? "privacy" : "terms"}
                      location="packet"
                      page="security"
                      className="text-base font-medium text-[#181613]"
                    >
                      {doc.label}
                    </EnterpriseCtaLink>
                  ) : (
                    <EnterpriseCtaLink
                      href={doc.href}
                      cta="dpa"
                      location="packet"
                      page="security"
                      className="text-base font-medium text-[#181613]"
                    >
                      {doc.label}
                    </EnterpriseCtaLink>
                  )}
                  <span className="text-[#4f4940]">{doc.note}</span>
                </li>
              ))}
            </ul>
          </section>

          <section className="pt-10 pb-4 md:pt-12">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              Incident response
            </h2>
            <p className="mt-5 text-base leading-7 text-[#4f4940]">
              Report a vulnerability privately — do not open a public GitHub
              issue. We acknowledge reports within 3 business days, keep you
              updated while we investigate, and credit you in the release notes
              if the report is accepted unless you prefer to stay anonymous.
            </p>
            <div className="mt-4 flex flex-col gap-2 text-sm">
              <EnterpriseCtaLink
                href={SECURITY_ADVISORY_URL}
                cta="security_report"
                location="packet"
                page="security"
              >
                Report a vulnerability on GitHub
              </EnterpriseCtaLink>
              <a
                href={`mailto:${SECURITY_REPORT_EMAIL}`}
                className="text-[#756b5d] underline decoration-[#d9cdb8] underline-offset-4 hover:text-[#181613]"
              >
                {SECURITY_REPORT_EMAIL}
              </a>
            </div>
          </section>

          <section className="pt-10 pb-4 md:pt-12">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              What ships, and how a pilot works
            </h2>
            <div className="mt-6 flex flex-col gap-6 text-sm leading-6 text-[#4f4940] md:flex-row">
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
            <p className="mt-8 text-base leading-7 text-[#4f4940]">
              {proofStatus.body}
            </p>
            <PilotPath />
          </section>

          <section className="pt-10 pb-20 text-center md:pt-12 md:pb-24">
            <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
              Request the packet
            </h2>
            <p className="mx-auto mt-5 max-w-lg text-base leading-7 text-[#4f4940]">
              Book a founder call, or email {PROCUREMENT_EMAIL} for a DPA and
              questionnaire responses grounded in this page.
            </p>
            <div className="mt-8 flex flex-col items-center gap-4">
              <BookFounderCall location="packet" page="security" />
              <EnterpriseCtaLink
                to="/enterprise/"
                cta="enterprise"
                location="packet"
                page="security"
              >
                Back to enterprise
              </EnterpriseCtaLink>
            </div>
          </section>
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}
