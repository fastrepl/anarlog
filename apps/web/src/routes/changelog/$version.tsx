import { createFileRoute, Link, notFound } from "@tanstack/react-router";

import { ChangelogContent } from "@anlg/changelog";

import { SiteFooter } from "@/components/site-footer";
import { formatChangelogDate, getChangelogEntry } from "@/lib/changelog";
import { getCanonicalUrl } from "@/lib/seo";

export const Route = createFileRoute("/changelog/$version")({
  component: Component,
  loader: async ({ params }) => {
    const entry = getChangelogEntry(params.version);
    if (!entry) {
      throw notFound();
    }
    return { entry };
  },
  head: ({ loaderData }) => {
    const entry = loaderData?.entry;
    if (!entry) return {};

    const url = getCanonicalUrl(`/changelog/${entry.version}`);
    const description =
      entry.summary ?? `Release notes for Anarlog v${entry.version}.`;

    return {
      links: [{ rel: "canonical", href: url }],
      meta: [
        // Per-version release notes are reference material for existing users,
        // not search targets. Indexing ~90 near-identical thin pages spends
        // crawl budget that belongs to the blog; /changelog/ stays the hub.
        { name: "robots", content: "noindex, follow" },
        { title: `Anarlog v${entry.version} Changelog` },
        {
          name: "description",
          content: description,
        },
        {
          property: "og:title",
          content: `Anarlog v${entry.version} Changelog`,
        },
        {
          property: "og:description",
          content: description,
        },
        { property: "og:url", content: url },
      ],
    };
  },
});

function Component() {
  const { entry } = Route.useLoaderData();

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[860px] px-5 py-8 md:px-8 md:py-12">
        <header className="flex items-center justify-between gap-6">
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <Link
          to="/changelog/"
          className="mt-16 inline-block text-sm text-[#756b5d] hover:text-[#181613]"
        >
          ← Changelog
        </Link>

        <header className="max-w-[760px] pt-10 pb-12 md:pt-14 md:pb-16">
          <div className="flex flex-wrap items-center gap-2 text-sm text-[#756b5d]">
            <span className="font-medium tracking-[0.14em] uppercase">
              Release notes
            </span>
            {entry.date && (
              <>
                <span aria-hidden="true">·</span>
                <time dateTime={entry.date}>
                  {formatChangelogDate(entry.date)}
                </time>
              </>
            )}
          </div>
          <h1 className="font-hand mt-5 text-5xl leading-[1.02] font-semibold tracking-normal text-balance text-black md:text-7xl">
            Anarlog v{entry.version}
          </h1>
          {entry.summary && (
            <p className="mt-6 max-w-[720px] text-xl leading-8 text-[#4f4940] md:text-2xl md:leading-9">
              {entry.summary}
            </p>
          )}
        </header>

        <article className="max-w-[760px] border-t border-[#eee8df] pt-10 md:pt-12">
          <ChangelogContent
            content={entry.content}
            className="changelog-prose"
          />
        </article>
      </div>

      <SiteFooter />
    </main>
  );
}
