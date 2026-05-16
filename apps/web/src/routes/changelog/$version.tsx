import { createFileRoute, Link, notFound } from "@tanstack/react-router";

import { ChangelogContent } from "@hypr/changelog";

import { SiteFooter } from "@/components/site-footer";
import { formatChangelogDate, getChangelogEntry } from "@/lib/changelog";
import { ANARLOG_SITE_URL } from "@/lib/seo";

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

    const url = `${ANARLOG_SITE_URL}/changelog/${entry.version}`;
    return {
      links: [{ rel: "canonical", href: url }],
      meta: [
        { title: `Anarlog v${entry.version} Changelog` },
        {
          name: "description",
          content: `Release notes for Anarlog v${entry.version}.`,
        },
        {
          property: "og:title",
          content: `Anarlog v${entry.version} Changelog`,
        },
        {
          property: "og:description",
          content: `Release notes for Anarlog v${entry.version}.`,
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
      <div className="mx-auto w-full max-w-[700px] px-5 py-8 md:px-8 md:py-12">
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

        <header className="pt-10 pb-12">
          <h1 className="font-hand text-5xl leading-[1.02] font-semibold tracking-normal text-balance text-black md:text-7xl">
            v{entry.version}
          </h1>
          {entry.date && (
            <time
              dateTime={entry.date}
              className="mt-6 block text-sm text-[#756b5d]"
            >
              {formatChangelogDate(entry.date)}
            </time>
          )}
        </header>

        <article className="border-t border-[#eee8df] pt-8">
          <ChangelogContent
            content={entry.content}
            className="text-sm leading-7"
          />
        </article>
      </div>

      <SiteFooter />
    </main>
  );
}
