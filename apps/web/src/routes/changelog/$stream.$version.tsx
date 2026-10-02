import { createFileRoute, Link, notFound } from "@tanstack/react-router";

import { ChangelogContent } from "@anlg/changelog";
import { ArrowLeft } from "@anlg/ui/components/icons";

import { ChangelogAvailability } from "@/components/changelog-availability";
import { SiteFooter } from "@/components/site-footer";
import { formatChangelogDate, getChangelogEntry } from "@/lib/changelog";
import { changelogStreamLabel, isChangelogStream } from "@/lib/changelog-path";
import { getCanonicalUrl } from "@/lib/seo";

export const Route = createFileRoute("/changelog/$stream/$version")({
  component: Component,
  loader: async ({ params }) => {
    const entry = isChangelogStream(params.stream)
      ? getChangelogEntry(params.stream, params.version)
      : undefined;
    if (!entry) {
      throw notFound();
    }
    return { entry };
  },
  head: ({ loaderData }) => {
    const entry = loaderData?.entry;
    if (!entry) return {};

    const url = getCanonicalUrl(`/changelog/${entry.stream}/${entry.version}`);
    const description =
      entry.summary ??
      `Release notes for Anarlog ${changelogStreamLabel(entry.stream)} v${entry.version}.`;

    return {
      links: [{ rel: "canonical", href: url }],
      meta: [
        // Per-version release notes are reference material for existing users,
        // not search targets. Indexing ~90 near-identical thin pages spends
        // crawl budget that belongs to the blog; /changelog/ stays the hub.
        { name: "robots", content: "noindex, follow" },
        {
          title: `Anarlog ${changelogStreamLabel(entry.stream)} v${entry.version} Changelog`,
        },
        {
          name: "description",
          content: description,
        },
        {
          property: "og:title",
          content: `Anarlog ${changelogStreamLabel(entry.stream)} v${entry.version} Changelog`,
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
    <main className="bg-surface text-fg min-h-screen">
      <div className="mx-auto w-full max-w-[860px] px-5 py-8 md:px-8 md:py-12">
        <header className="flex items-center justify-between gap-6">
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <Link
          to="/changelog/"
          search={{ stream: entry.stream }}
          className="text-brand-dark hover:text-fg mt-16 inline-flex items-center gap-1 text-sm"
        >
          <ArrowLeft size={14} aria-hidden="true" />
          Changelog
        </Link>

        <header className="max-w-[760px] pt-10 pb-12 md:pt-14 md:pb-16">
          <div className="text-brand-dark flex flex-wrap items-center gap-2 text-sm">
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
          <h1 className="font-hand text-fg mt-5 text-5xl leading-[1.02] font-semibold tracking-normal text-balance md:text-7xl">
            Anarlog {changelogStreamLabel(entry.stream)} v{entry.version}
          </h1>
          <ChangelogAvailability entry={entry} />
          {entry.summary && (
            <p className="text-brand-dark mt-6 max-w-[720px] text-xl leading-8 md:text-2xl md:leading-9">
              {entry.summary}
            </p>
          )}
        </header>

        <article className="border-border-subtle max-w-[760px] border-t pt-10 md:pt-12">
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
