import { createFileRoute, Link } from "@tanstack/react-router";
import allArticleSummaries from "article-summaries";

import { SiteFooter } from "@/components/site-footer";
import { transcriptionGuideSections } from "@/lib/blog-related";
import {
  DEFAULT_OG_IMAGE_URL,
  getBreadcrumbListJsonLd,
  getCanonicalUrl,
  getStructuredDataGraph,
} from "@/lib/seo";

const title =
  "Transcription Guides: Tools, Local Models & Meeting Setup | Anarlog";
const description =
  "Find the right transcription workflow for files and meetings. Explore free tools, Mac and Windows options, offline models, and Zoom, Meet, and Teams setup.";
const url = getCanonicalUrl("/guides/transcription");

export const Route = createFileRoute("/guides/transcription/")({
  component: TranscriptionGuides,
  loader: () =>
    transcriptionGuideSections.map((section) => ({
      ...section,
      articles: section.slugs.map((slug) => {
        const article = allArticleSummaries.find(
          (article) => article.slug === slug,
        );
        if (!article) {
          throw new Error(`Missing transcription guide: ${slug}`);
        }
        return article;
      }),
    })),
  head: () => ({
    links: [{ rel: "canonical", href: url }],
    meta: [
      { title },
      { name: "description", content: description },
      { property: "og:title", content: title },
      { property: "og:description", content: description },
      { property: "og:url", content: url },
      { property: "og:type", content: "website" },
      { property: "og:image", content: DEFAULT_OG_IMAGE_URL },
      { name: "twitter:title", content: title },
      { name: "twitter:description", content: description },
      { name: "twitter:image", content: DEFAULT_OG_IMAGE_URL },
    ],
    scripts: [
      {
        type: "application/ld+json",
        children: JSON.stringify(
          getStructuredDataGraph([
            { "@type": "CollectionPage", url, name: title, description },
            getBreadcrumbListJsonLd([
              { name: "Home", item: getCanonicalUrl() },
              { name: "Transcription guides", item: url },
            ]),
          ]),
        ),
      },
    ],
  }),
});

function TranscriptionGuides() {
  const sections = Route.useLoaderData();

  return (
    <main className="bg-surface text-fg min-h-screen">
      <div className="mx-auto w-full max-w-[860px] px-5 py-8 md:px-8 md:py-12">
        <header className="flex items-center justify-between gap-6">
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
          <Link to="/blog/" className="text-sm hover:underline">
            All articles
          </Link>
        </header>

        <section className="pt-24 pb-16 md:pt-32">
          <p className="text-brand-dark mb-4 text-sm">Anarlog guides</p>
          <h1 className="text-4xl leading-[1.06] font-semibold tracking-[-0.03em] text-balance md:text-6xl">
            Find your transcription workflow.
          </h1>
          <p className="text-brand-dark mt-6 max-w-2xl text-lg leading-8">
            An interview file, a live call, and an offline meeting need
            different setups. Start with the recording you have, where it can be
            processed, and what you need to do with the transcript.
          </p>
          <nav
            aria-label="Transcription topics"
            className="mt-8 flex flex-wrap gap-3"
          >
            {sections.map((section) => (
              <a
                key={section.id}
                href={`#${section.id}`}
                className="border-border-subtle hover:bg-surface-subtle rounded-full border px-4 py-2 text-sm"
              >
                {section.title}
              </a>
            ))}
          </nav>
        </section>

        <div className="space-y-16">
          {sections.map((section) => (
            <section key={section.id} id={section.id} className="scroll-mt-8">
              <h2 className="text-2xl leading-tight font-semibold tracking-tight md:text-3xl">
                {section.title}
              </h2>
              <p className="text-brand-dark mt-3 max-w-2xl leading-7">
                {section.description}
              </p>
              <ul className="mt-6 grid gap-4 sm:grid-cols-2">
                {section.articles.map((article) => (
                  <li key={article.slug}>
                    <Link
                      to="/blog/$slug/"
                      params={{ slug: article.slug }}
                      className="border-border-subtle hover:bg-surface-subtle block h-full rounded-lg border p-5 transition-colors"
                    >
                      <h3 className="text-lg leading-snug font-semibold tracking-tight">
                        {article.title}
                      </h3>
                      <p className="text-brand-dark mt-3 text-sm leading-6">
                        {article.meta_description}
                      </p>
                      <span className="mt-4 inline-block text-sm underline">
                        Read guide
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>

        <section className="border-border-subtle mt-16 border-t pt-10 pb-8">
          <h2 className="text-2xl leading-tight font-semibold tracking-tight md:text-3xl">
            Bring the transcript into your meeting notes.
          </h2>
          <p className="text-brand-dark mt-4 max-w-2xl leading-7">
            Anarlog captures meetings without a bot and keeps the meeting record
            locally. Choose supported on-device models, your own provider keys,
            or hosted AI. Built-in local speech models require Apple Silicon;
            check your platform before choosing an offline setup.
          </p>
          <div className="mt-6 flex flex-wrap items-center gap-5">
            <Link
              to="/download/"
              className="rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-6 py-3 text-sm text-white"
            >
              Try Anarlog
            </Link>
            <a
              href="https://docs.anarlog.so/desktop-installation"
              className="text-sm underline"
            >
              Check platform requirements
            </a>
          </div>
        </section>
      </div>
      <SiteFooter />
    </main>
  );
}
