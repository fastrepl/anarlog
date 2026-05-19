import { createFileRoute, Link } from "@tanstack/react-router";

import { SiteFooter } from "@/components/site-footer";
import { changelogEntries, formatChangelogDate } from "@/lib/changelog";
import { ANARLOG_SITE_URL } from "@/lib/seo";

export const Route = createFileRoute("/changelog/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: `${ANARLOG_SITE_URL}/changelog` }],
    meta: [
      { title: "Anarlog Changelog" },
      {
        name: "description",
        content:
          "See the latest Anarlog desktop app updates, fixes, and product changes.",
      },
      { property: "og:title", content: "Anarlog Changelog" },
      { property: "og:url", content: `${ANARLOG_SITE_URL}/changelog` },
    ],
  }),
});

function Component() {
  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 py-8 md:px-8 md:py-12">
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
          <ol className="grid gap-12">
            {changelogEntries.map((entry) => (
              <li
                key={entry.version}
                id={entry.version}
                className="scroll-mt-8 border-t border-[#eee8df] pt-8"
              >
                <article className="grid gap-4">
                  <header className="flex flex-wrap items-baseline justify-between gap-x-5 gap-y-2">
                    <Link
                      to="/changelog/$version/"
                      params={{ version: entry.version }}
                      className="group"
                    >
                      <h2 className="font-hand text-4xl leading-none font-semibold tracking-normal text-[#756b5d] group-hover:text-[#4f4940]">
                        v{entry.version}
                      </h2>
                    </Link>
                    {entry.date && (
                      <time
                        dateTime={entry.date}
                        className="text-sm text-[#756b5d]"
                      >
                        {formatChangelogDate(entry.date)}
                      </time>
                    )}
                  </header>
                  <p className="leading-7 text-[#4f4940]">
                    {entry.summary ?? getEntrySummary(entry.content)}
                  </p>
                  <Link
                    to="/changelog/$version/"
                    params={{ version: entry.version }}
                    className="text-sm font-medium text-[#756b5d] hover:text-[#181613]"
                  >
                    Read release notes
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

function getEntrySummary(content: string) {
  return (
    content
      .split("\n")
      .map((line) => line.trim())
      .find(
        (line) => line && !line.startsWith("#") && !line.startsWith("!["),
      ) ?? "See what changed in this release."
  );
}
