import { createFileRoute, Link } from "@tanstack/react-router";

import { ArrowRight } from "@anlg/ui/components/icons";

import { SiteFooter } from "@/components/site-footer";
import { changelogEntries, formatChangelogDate } from "@/lib/changelog";
import { getEntrySummary } from "@/lib/changelog-summary";
import { getCanonicalUrl } from "@/lib/seo";

export const Route = createFileRoute("/changelog/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: getCanonicalUrl("/changelog") }],
    meta: [
      { title: "Anarlog Changelog" },
      {
        name: "description",
        content:
          "See the latest Anarlog desktop app updates, fixes, and product changes.",
      },
      { property: "og:title", content: "Anarlog Changelog" },
      { property: "og:url", content: getCanonicalUrl("/changelog") },
    ],
  }),
});

function Component() {
  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[860px] px-5 py-8 md:px-8 md:py-12">
        <header className="flex items-center justify-between gap-6">
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <section className="pt-24 pb-16 md:pt-32">
          <h1 className="font-hand text-6xl leading-[0.98] font-semibold tracking-normal text-balance text-black md:text-8xl">
            Changelog
          </h1>
          <p className="mt-6 max-w-2xl text-xl leading-9 text-[#363029]">
            Product updates, fixes, and release notes for Anarlog.
          </p>
        </section>

        {changelogEntries.length > 0 ? (
          <ol className="border-y border-[#eee8df]">
            {changelogEntries.map((entry, index) => (
              <li
                key={entry.version}
                id={entry.version}
                className="scroll-mt-8 border-b border-[#eee8df] last:border-b-0"
              >
                <article>
                  <Link
                    to="/changelog/$version/"
                    params={{ version: entry.version }}
                    className="group grid gap-4 py-7 sm:grid-cols-[10rem_minmax(0,1fr)_1.5rem] sm:items-start sm:gap-6 md:py-9"
                  >
                    <header>
                      <div className="flex flex-wrap items-center gap-2.5">
                        <h2 className="font-hand text-4xl leading-none font-semibold tracking-normal text-[#756b5d] transition-colors group-hover:text-[#181613]">
                          v{entry.version}
                        </h2>
                        {index === 0 && (
                          <span className="rounded-full bg-[#f3eee6] px-2 py-1 text-[0.65rem] font-semibold tracking-[0.12em] text-[#756b5d] uppercase">
                            Latest
                          </span>
                        )}
                      </div>
                      {entry.date && (
                        <time
                          dateTime={entry.date}
                          className="mt-2 block text-xs text-[#756b5d]"
                        >
                          {formatChangelogDate(entry.date)}
                        </time>
                      )}
                    </header>
                    <p className="text-base leading-7 text-[#4f4940] transition-colors group-hover:text-[#363029] md:text-lg md:leading-8">
                      {getEntrySummary(entry.summary ?? entry.content)}
                    </p>
                    <ArrowRight
                      aria-hidden="true"
                      className="mt-1 hidden text-[#9a9082] transition group-hover:translate-x-1 group-hover:text-[#181613] sm:block"
                      size={20}
                    />
                  </Link>
                </article>
              </li>
            ))}
          </ol>
        ) : (
          <p className="border-t border-[#eee8df] pt-8 text-[#4f4940]">
            No changelog entries yet.
          </p>
        )}
      </div>

      <SiteFooter />
    </main>
  );
}
