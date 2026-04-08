import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Download } from "lucide-react";

import { ChangelogContent, fixImageUrls } from "@hypr/changelog";
import { cn } from "@hypr/utils";

import { type ChangelogWithMeta, getChangelogList } from "@/changelog";
import { CTASection } from "@/components/cta-section";
import { getDownloadLinks, groupDownloadLinks } from "@/utils/download";

export const Route = createFileRoute("/_view/changelog/")({
  component: Component,
  loader: async () => {
    const changelogs = getChangelogList();
    return { changelogs };
  },
  head: () => ({
    meta: [
      { title: "Changelog - Char" },
      {
        name: "description",
        content: "Track every update, improvement, and fix to Char",
      },
      { property: "og:title", content: "Changelog - Char" },
      {
        property: "og:description",
        content: "Track every update, improvement, and fix to Char",
      },
      { property: "og:type", content: "website" },
      { property: "og:url", content: "https://char.com/changelog" },
    ],
  }),
});

function Component() {
  const { changelogs } = Route.useLoaderData();

  return (
    <main className="min-h-screen flex-1">
      <div className="mx-auto">
        <div className="px-4 py-16 lg:py-24">
          <HeroSection />
        </div>
        <div className="mt-16">
          {changelogs.map((changelog, index) => (
            <div key={changelog.slug}>
              <div className="mx-auto max-w-4xl px-4">
                <ChangelogSection changelog={changelog} />
              </div>
              {index < changelogs.length - 1 && (
                <div className="my-12 border-b border-neutral-100" />
              )}
            </div>
          ))}
        </div>
        <CTASection />
      </div>
    </main>
  );
}

function HeroSection() {
  return (
    <div className="text-left">
      <h1 className="text-color mb-6 font-mono text-4xl tracking-tight sm:text-5xl">
        Changelog
      </h1>
      <p className="text-fg-muted text-lg sm:text-xl">
        Track every update, improvement, and fix to Char
      </p>
    </div>
  );
}

function ChangelogSection({ changelog }: { changelog: ChangelogWithMeta }) {
  return (
    <section className="grid grid-cols-1 gap-6 md:grid-cols-[160px_1fr] md:gap-12">
      <div className="flex flex-col gap-6 md:sticky md:top-24 md:self-start">
        <div className="flex flex-col gap-1">
          <Link to="/changelog/$slug/" params={{ slug: changelog.slug }}>
            <h2 className="cursor-pointer font-mono text-4xl font-medium text-stone-700 transition-colors hover:text-stone-900">
              {changelog.version}
            </h2>
          </Link>
          <time
            className="mt-1 text-sm text-neutral-500"
            dateTime={changelog.date}
          >
            {new Date(changelog.date).toLocaleDateString("en-US", {
              year: "numeric",
              month: "long",
              day: "numeric",
            })}
          </time>
        </div>

        <DownloadLinks version={changelog.version} />
      </div>

      <div>
        <article className="max-w-none text-sm [&_h2]:font-mono [&_h3]:font-mono">
          <ChangelogContent content={fixImageUrls(changelog.content)} />
        </article>

        <Link
          to="/changelog/$slug/"
          params={{ slug: changelog.slug }}
          className="mt-4 inline-flex items-center gap-1 text-sm text-stone-600 transition-colors hover:text-stone-900"
        >
          Read more
          <Icon icon="mdi:arrow-right" className="text-base" />
        </Link>
      </div>
    </section>
  );
}

function DownloadLinks({ version }: { version: string }) {
  const links = getDownloadLinks(version);
  const grouped = groupDownloadLinks(links);

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h3 className="mb-2 flex items-center gap-1.5 text-xs font-medium tracking-wider text-stone-500 uppercase">
          <Icon icon="simple-icons:apple" className="text-sm" />
          macOS
        </h3>
        <div className="flex flex-col gap-1.5">
          {grouped.macos.map((link) => (
            <a
              key={link.url}
              href={link.url}
              className={cn([
                "flex h-8 items-center gap-2 rounded-full px-4 text-sm transition-all",
                "bg-linear-to-b from-white to-stone-50 text-neutral-700",
                "border border-neutral-300",
                "hover:scale-[102%] hover:shadow-xs active:scale-[98%]",
              ])}
            >
              <Download className="size-3.5 shrink-0" />
              <span className="truncate">{link.label}</span>
            </a>
          ))}
        </div>
      </div>
    </div>
  );
}
