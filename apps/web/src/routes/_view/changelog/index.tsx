import { MDXContent } from "@content-collections/mdx/react";
import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Download } from "lucide-react";
import semver from "semver";

import { cn } from "@hypr/utils";

import { type ChangelogWithMeta, getChangelogList } from "@/changelog";
import { defaultMDXComponents } from "@/components/mdx";
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
    <main
      className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <div className="px-6 py-16 lg:py-24">
          <HeroSection />
        </div>
        <div className="mt-16">
          {changelogs.map((changelog, index) => (
            <div key={changelog.slug}>
              <div className="mx-auto max-w-4xl px-6">
                <ChangelogSection changelog={changelog} />
              </div>
              {index < changelogs.length - 1 && (
                <div className="my-12 border-b border-neutral-100" />
              )}
            </div>
          ))}
        </div>
        <div className="px-6 pb-16 lg:pb-24"></div>
      </div>
    </main>
  );
}

function HeroSection() {
  return (
    <div className="text-center">
      <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
        Changelog
      </h1>
      <p className="text-lg text-neutral-600 sm:text-xl">
        Track every update, improvement, and fix to Char
      </p>
    </div>
  );
}

function ChangelogSection({ changelog }: { changelog: ChangelogWithMeta }) {
  const currentVersion = semver.parse(changelog.version);
  const isPrerelease = currentVersion && currentVersion.prerelease.length > 0;
  const nightlyNumber =
    isPrerelease && currentVersion?.prerelease[0] === "nightly"
      ? currentVersion.prerelease[1]
      : null;

  return (
    <section className="grid grid-cols-1 gap-6 md:grid-cols-[160px_1fr] md:gap-12">
      <div className="flex flex-col gap-6 md:sticky md:top-24 md:self-start">
        <div className="flex flex-col gap-1">
          {!isPrerelease && (
            <span className="inline-flex w-fit items-center gap-1 rounded-full bg-linear-to-t from-stone-200 to-stone-100 px-1.5 py-0.5 text-xs font-medium text-stone-700">
              <Icon icon="ri:rocket-fill" className="text-xs" />
              Stable
            </span>
          )}
          {isPrerelease && (
            <div className="flex items-center gap-1.5">
              <span className="inline-flex items-center gap-1 rounded-full bg-linear-to-b from-[#03BCF1] to-[#127FE5] px-1.5 py-0.5 text-xs font-medium text-white">
                <Icon icon="ri:moon-fill" className="text-xs" />
                Beta
              </span>
              {nightlyNumber !== null && (
                <span className="inline-flex items-center rounded-full bg-stone-100 px-1.5 py-0.5 text-xs font-medium text-stone-600">
                  #{nightlyNumber}
                </span>
              )}
            </div>
          )}
          <Link to="/changelog/$slug/" params={{ slug: changelog.slug }}>
            <h2 className="cursor-pointer font-mono text-4xl font-medium text-stone-700 transition-colors hover:text-stone-900">
              {currentVersion
                ? `${currentVersion.major}.${currentVersion.minor}.${currentVersion.patch}`
                : changelog.version}
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
        <article className="prose prose-stone prose-sm prose-headings:font-serif prose-headings:font-semibold prose-h2:text-lg prose-h2:mt-4 prose-h2:mb-2 prose-h3:text-base prose-h3:mt-3 prose-h3:mb-1 prose-ul:my-2 prose-li:my-0.5 prose-a:text-stone-600 prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-neutral-200 prose-code:rounded prose-code:px-1 prose-code:py-0.5 prose-code:text-xs prose-code:font-mono prose-code:text-stone-700 prose-img:rounded prose-img:border prose-img:border-neutral-200 prose-img:my-3 max-w-none">
          <MDXContent code={changelog.mdx} components={defaultMDXComponents} />
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

      <div>
        <h3 className="mb-2 flex items-center gap-1.5 text-xs font-medium tracking-wider text-stone-500 uppercase">
          <Icon icon="simple-icons:linux" className="text-sm" />
          Linux
        </h3>
        <div className="flex flex-col gap-1.5">
          {grouped.linux.map((link) => (
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
