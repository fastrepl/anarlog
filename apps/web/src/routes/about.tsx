import { createFileRoute, Link } from "@tanstack/react-router";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { SiteFooter } from "@/components/site-footer";
import {
  ABOUT_UPDATED_ON,
  differentiators,
  faqs,
  founders,
  howAnarlogWorks,
  keyFacts,
  originStory,
  VALUE_PROP,
  whatAnarlogDoes,
  whoUsesAnarlog,
} from "@/lib/about";
import {
  ANARLOG_SITE_URL,
  getCanonicalUrl,
  getStructuredDataGraph,
} from "@/lib/seo";

const title = "About · Anarlog";
const description =
  "Who builds Anarlog, what it does, how it differs from Granola, Otter, and Fathom, who uses it, and the key facts about the company.";

export const Route = createFileRoute("/about")({
  component: AboutPage,
  head: () => ({
    meta: [
      { title },
      { name: "description", content: description },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: getCanonicalUrl("/about") },
      { name: "twitter:title", content: title },
      { name: "twitter:description", content: description },
      { name: "twitter:url", content: getCanonicalUrl("/about") },
    ],
    links: [{ rel: "canonical", href: getCanonicalUrl("/about") }],
    scripts: [
      {
        type: "application/ld+json",
        children: JSON.stringify(
          getStructuredDataGraph([
            {
              "@type": "Organization",
              name: "Anarlog",
              legalName: "Fastrepl, Inc.",
              url: getCanonicalUrl(),
              logo: `${ANARLOG_SITE_URL}/logo.svg`,
              foundingDate: "2023",
              founder: founders.map((founder) => ({
                "@type": "Person",
                name: founder.name,
                jobTitle: founder.role,
                sameAs: founder.links.map((link) => link.href),
              })),
              location: [
                { "@type": "Place", name: "Seoul, South Korea" },
                { "@type": "Place", name: "San Francisco, California" },
              ],
              contactPoint: {
                "@type": "ContactPoint",
                email: "team@fastrepl.com",
                contactType: "customer support",
              },
              sameAs: [
                "https://github.com/fastrepl/anarlog",
                "https://x.com/anarlogapp",
                "https://www.linkedin.com/company/anarlog/",
                "https://www.reddit.com/r/anarlog/",
              ],
            },
            {
              "@type": "FAQPage",
              mainEntity: faqs.map((faq) => ({
                "@type": "Question",
                name: faq.question,
                acceptedAnswer: { "@type": "Answer", text: faq.answer },
              })),
            },
          ]),
        ),
      },
    ],
  }),
});

function SectionHeading({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
      {children}
    </h2>
  );
}

function AboutPage() {
  const updatedOn = new Date(
    `${ABOUT_UPDATED_ON}T00:00:00Z`,
  ).toLocaleDateString("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  });

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 pt-4 pb-8 md:px-8 md:pt-4 md:pb-12">
        <section className="pt-10 pb-4 text-center md:pt-12 md:pb-6">
          <Link to="/" aria-label="Anarlog home" className="inline-flex">
            <AnarlogLogo className="h-8 w-auto md:h-9" />
          </Link>
          <h1 className="font-hand mt-12 text-4xl leading-none font-semibold text-[#181613] md:mt-16 md:text-5xl">
            About Anarlog
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
            {VALUE_PROP}
          </p>
          <p className="mt-3 text-xs text-[#756b5d]">
            Last updated {updatedOn}
          </p>
        </section>

        <section className="pt-10 pb-4 md:pt-12">
          <SectionHeading>What Anarlog does</SectionHeading>
          <div className="mt-6 flex flex-col gap-6">
            {whatAnarlogDoes.map((item) => (
              <div key={item.title}>
                <h3 className="text-base font-medium text-[#181613]">
                  {item.title}
                </h3>
                <p className="mt-1 text-sm leading-6 text-[#4f4940]">
                  {item.body}
                </p>
              </div>
            ))}
          </div>
        </section>

        <section className="pt-10 pb-4 md:pt-12">
          <SectionHeading>What makes Anarlog different</SectionHeading>
          <p className="mt-5 text-base leading-7 text-[#4f4940]">
            The full feature-by-feature comparison against twelve notetakers is
            on the{" "}
            <Link to="/pricing/" className="underline">
              pricing page
            </Link>
            .
          </p>
          <ol className="mt-8 flex flex-col gap-4">
            {differentiators.map((item, index) => (
              <li
                key={item.title}
                className="rounded-2xl border border-[#eadfce] bg-[#fffaf0] px-5 py-4"
              >
                <p className="text-xs tracking-[0.04em] text-[#756b5d]">
                  {String(index + 1).padStart(2, "0")}
                </p>
                <h3 className="mt-1 text-base font-medium text-[#181613]">
                  {item.title}
                </h3>
                <p className="mt-1 text-sm leading-6 text-[#4f4940]">
                  {item.body}
                </p>
              </li>
            ))}
          </ol>
        </section>

        <section className="pt-10 pb-4 md:pt-12">
          <SectionHeading>Who uses Anarlog</SectionHeading>
          <ul className="mt-6 list-disc space-y-2 pl-5 text-sm leading-6 text-[#4f4940]">
            {whoUsesAnarlog.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </section>

        <section className="pt-10 pb-4 md:pt-12">
          <SectionHeading>The team behind Anarlog</SectionHeading>
          {originStory.map((paragraph) => (
            <p
              key={paragraph}
              className="mt-5 text-base leading-7 text-[#4f4940]"
            >
              {paragraph}
            </p>
          ))}
          <div className="mt-8 grid gap-4 sm:grid-cols-2">
            {founders.map((founder) => (
              <div
                key={founder.name}
                className="rounded-2xl border border-[#eadfce] px-5 py-4"
              >
                <h3 className="text-base font-medium text-[#181613]">
                  {founder.name}
                </h3>
                <p className="text-xs tracking-[0.04em] text-[#756b5d]">
                  {founder.role}
                </p>
                <p className="mt-2 text-sm leading-6 text-[#4f4940]">
                  {founder.bio}
                </p>
                <div className="mt-3 flex gap-3 text-xs">
                  {founder.links.map((link) => (
                    <a
                      key={link.href}
                      href={link.href}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="underline hover:text-[#181613]"
                    >
                      {link.label}
                    </a>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="pt-10 pb-4 md:pt-12">
          <SectionHeading>How Anarlog works</SectionHeading>
          <div className="mt-6 flex flex-col gap-6">
            {howAnarlogWorks.map((item) => (
              <div key={item.title}>
                <h3 className="text-base font-medium text-[#181613]">
                  {item.title}
                </h3>
                <p className="mt-1 text-sm leading-6 text-[#4f4940]">
                  {item.body}
                </p>
              </div>
            ))}
          </div>
        </section>

        <section className="pt-10 pb-4 md:pt-12">
          <SectionHeading>Key facts</SectionHeading>
          <dl className="mt-6 divide-y divide-[#eadfce] border-y border-[#eadfce] text-sm">
            {keyFacts.map((fact) => (
              <div
                key={fact.term}
                className="grid gap-1 py-3 sm:grid-cols-[10rem_1fr] sm:gap-4"
              >
                <dt className="font-medium text-[#181613]">{fact.term}</dt>
                <dd className="leading-6 text-[#4f4940]">{fact.detail}</dd>
              </div>
            ))}
          </dl>
        </section>

        <section className="pt-10 pb-20 md:pt-12 md:pb-24">
          <SectionHeading>Frequently asked questions</SectionHeading>
          <div className="mt-6 flex flex-col gap-6">
            {faqs.map((faq) => (
              <div key={faq.question}>
                <h3 className="text-base font-medium text-[#181613]">
                  {faq.question}
                </h3>
                <p className="mt-1 text-sm leading-6 text-[#4f4940]">
                  {faq.answer}
                </p>
              </div>
            ))}
          </div>
        </section>
      </div>

      <SiteFooter />
    </main>
  );
}
