import { createFileRoute, Link } from "@tanstack/react-router";

import { ArrowRight } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { ChangelogAvailability } from "@/components/changelog-availability";
import { SiteFooter } from "@/components/site-footer";
import { changelogEntries, formatChangelogDate } from "@/lib/changelog";
import {
  type ChangelogStream,
  changelogStreams,
  changelogStreamLabel,
  isChangelogStream,
} from "@/lib/changelog-path";
import { getEntrySummary } from "@/lib/changelog-summary";
import { getCanonicalUrl } from "@/lib/seo";

export const Route = createFileRoute("/changelog/")({
  component: Component,
  validateSearch: (
    search: Record<string, unknown>,
  ): { stream?: ChangelogStream } => ({
    stream: isChangelogStream(search.stream) ? search.stream : undefined,
  }),
  head: () => ({
    links: [{ rel: "canonical", href: getCanonicalUrl("/changelog") }],
    meta: [
      { title: "Anarlog Changelog" },
      {
        name: "description",
        content:
          "See the latest Anarlog desktop and mobile app updates, fixes, and product changes.",
      },
      { property: "og:title", content: "Anarlog Changelog" },
      { property: "og:url", content: getCanonicalUrl("/changelog") },
    ],
  }),
});

function Component() {
  const { stream } = Route.useSearch();
  const entries = changelogEntries.filter(
    (entry) => !stream || entry.stream === stream,
  );
  return (
    <main className="bg-surface text-fg min-h-screen">
      <div className="mx-auto w-full max-w-[860px] px-5 py-8 md:px-8 md:py-12">
        <header className="flex items-center justify-between gap-6">
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <section className="pt-24 pb-16 md:pt-32">
          <h1 className="font-hand text-fg text-6xl leading-[0.98] font-semibold tracking-normal text-balance md:text-8xl">
            Changelog
          </h1>
          <p className="text-brand-dark mt-6 max-w-2xl text-xl leading-9">
            Product updates, fixes, and release notes for Anarlog.
          </p>
        </section>

        <nav
          aria-label="Changelog platform"
          className="mb-8 flex flex-wrap gap-2"
        >
          {[undefined, ...changelogStreams].map((filter) => (
            <Link
              key={filter ?? "all"}
              to="/changelog/"
              search={{ stream: filter }}
              aria-current={stream === filter ? "page" : undefined}
              className={cn([
                "rounded-full border px-4 py-2 text-sm transition-colors",
                stream === filter
                  ? "border-brand-dark bg-brand-dark text-white"
                  : "border-border-subtle text-brand-dark hover:bg-surface-subtle",
              ])}
            >
              {filter ? changelogStreamLabel(filter) : "All updates"}
            </Link>
          ))}
        </nav>

        {entries.length > 0 ? (
          <ol className="border-border-subtle border-y">
            {entries.map((entry) => (
              <li
                key={`${entry.stream}/${entry.version}`}
                id={
                  entry.stream === "desktop"
                    ? entry.version
                    : `${entry.stream}-${entry.version}`
                }
                className="border-border-subtle scroll-mt-8 border-b last:border-b-0"
              >
                <article>
                  <Link
                    to="/changelog/$stream/$version/"
                    params={{ stream: entry.stream, version: entry.version }}
                    className="group grid gap-4 py-7 sm:grid-cols-[10rem_minmax(0,1fr)_1.5rem] sm:items-start sm:gap-6 md:py-9"
                  >
                    <header>
                      <p className="text-brand-dark mb-2 text-xs font-semibold">
                        {changelogStreamLabel(entry.stream)}
                      </p>
                      <div className="flex flex-wrap items-center gap-2.5">
                        <h2 className="font-hand text-brand-dark group-hover:text-fg text-4xl leading-none font-semibold tracking-normal transition-colors">
                          v{entry.version}
                        </h2>
                        {changelogEntries.find(
                          (candidate) => candidate.stream === entry.stream,
                        ) === entry && (
                          <span className="bg-surface-subtle text-brand-dark rounded-full px-2 py-1 text-[0.65rem] font-semibold tracking-[0.12em] uppercase">
                            Latest
                          </span>
                        )}
                      </div>
                      {entry.date && (
                        <time
                          dateTime={entry.date}
                          className="text-brand-dark mt-2 block text-xs"
                        >
                          {formatChangelogDate(entry.date)}
                        </time>
                      )}
                      <ChangelogAvailability entry={entry} />
                    </header>
                    <p className="text-brand-dark group-hover:text-brand-dark text-base leading-7 transition-colors md:text-lg md:leading-8">
                      {getEntrySummary(entry.summary ?? entry.content)}
                    </p>
                    <ArrowRight
                      aria-hidden="true"
                      className="text-brand-dark group-hover:text-fg mt-1 hidden transition group-hover:translate-x-1 sm:block"
                      size={20}
                    />
                  </Link>
                </article>
              </li>
            ))}
          </ol>
        ) : (
          <p className="border-border-subtle text-brand-dark border-t pt-8">
            {stream
              ? `No ${changelogStreamLabel(stream).toLowerCase()} release notes yet.`
              : "No changelog entries yet."}
          </p>
        )}
      </div>

      <SiteFooter />
    </main>
  );
}
