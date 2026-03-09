import { MDXContent } from "@content-collections/mdx/react";
import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { Download } from "lucide-react";
import semver from "semver";

import { cn } from "@hypr/utils";

import {
  type ChangelogWithMeta,
  getChangelogBySlug,
  getChangelogList,
} from "@/changelog";
import { defaultMDXComponents } from "@/components/mdx";
import { NotFoundContent } from "@/components/not-found";
import { getDownloadLinks, groupDownloadLinks } from "@/utils/download";

export const Route = createFileRoute("/_view/changelog/$slug")({
  component: Component,
  notFoundComponent: NotFoundContent,
  loader: async ({ params }) => {
    const changelog = getChangelogBySlug(params.slug);
    if (!changelog) {
      throw notFound();
    }

    const allChangelogs = getChangelogList();

    const beforeVersion = changelog.beforeVersion;
    const diffUrl =
      beforeVersion != null
        ? `https://github.com/fastrepl/char/compare/desktop_v${beforeVersion}...desktop_v${changelog.version}`
        : null;

    return {
      changelog,
      allChangelogs,
      diffUrl,
    };
  },
  head: ({ loaderData }) => {
    if (!loaderData) return {};

    const { changelog } = loaderData;
    const currentVersion = semver.parse(changelog.version);
    const isNightly = currentVersion && currentVersion.prerelease.length > 0;

    const title = `Version ${changelog.version} - Char Changelog`;
    const description = `Explore what's new in Char version ${changelog.version}${isNightly ? " (Nightly)" : ""}.`;
    const url = `https://char.com/changelog/${changelog.slug}`;
    const ogImageUrl = `https://char.com/og?type=changelog&version=${encodeURIComponent(changelog.version)}&v=1`;

    return {
      meta: [
        { title },
        { name: "description", content: description },
        { property: "og:type", content: "article" },
        { property: "og:title", content: title },
        { property: "og:description", content: description },
        { property: "og:url", content: url },
        { property: "og:image", content: ogImageUrl },
        { property: "og:image:width", content: "1200" },
        { property: "og:image:height", content: "630" },
        { name: "twitter:card", content: "summary_large_image" },
        { name: "twitter:site", content: "@getcharnotes" },
        { name: "twitter:creator", content: "@getcharnotes" },
        { name: "twitter:title", content: title },
        { name: "twitter:description", content: description },
        { name: "twitter:url", content: url },
        { name: "twitter:image", content: ogImageUrl },
      ],
    };
  },
});

function Component() {
  const { changelog, allChangelogs, diffUrl } = Route.useLoaderData();

  const currentVersion = semver.parse(changelog.version);
  const isPrerelease = !!(
    currentVersion && currentVersion.prerelease.length > 0
  );

  return (
    <main
      className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <div className="mx-auto max-w-3xl px-6 pt-16 pb-8 lg:pt-24">
          <div className="hidden gap-12 md:flex md:flex-col md:items-center">
            <div className="flex flex-col items-center gap-6">
              <img
                src={
                  isPrerelease
                    ? "/api/images/icons/nightly-icon.png"
                    : "/api/images/icons/stable-icon.png"
                }
                alt="Char"
                className="size-32 rounded-2xl"
              />
              <div className="flex flex-col items-center gap-2">
                <h1 className="font-mono text-3xl font-medium text-stone-700 sm:text-4xl">
                  {changelog.version}
                </h1>
                <time
                  className="text-sm text-neutral-500"
                  dateTime={changelog.date}
                >
                  {new Date(changelog.date).toLocaleDateString("en-US", {
                    year: "numeric",
                    month: "long",
                    day: "numeric",
                  })}
                </time>
              </div>
            </div>

            <DownloadLinksHero version={changelog.version} />
          </div>

          <div className="text-center md:hidden">
            <div className="mb-8 flex flex-col items-center gap-3">
              <img
                src={
                  isPrerelease
                    ? "/api/images/icons/nightly-icon.png"
                    : "/api/images/icons/stable-icon.png"
                }
                alt="Char"
                className="size-16 rounded-2xl"
              />
              <div className="flex flex-col items-center gap-2">
                <h1 className="font-mono text-3xl font-medium text-stone-700">
                  {changelog.version}
                </h1>
                <time
                  className="text-sm text-neutral-500"
                  dateTime={changelog.date}
                >
                  {new Date(changelog.date).toLocaleDateString("en-US", {
                    year: "numeric",
                    month: "long",
                    day: "numeric",
                  })}
                </time>
              </div>
            </div>

            <DownloadLinksHeroMobile version={changelog.version} />
          </div>

          <article className="prose prose-stone prose-headings:font-serif prose-headings:font-semibold prose-h2:text-2xl prose-h2:mt-8 prose-h2:mb-4 prose-h3:text-xl prose-h3:mt-6 prose-h3:mb-3 prose-h4:text-lg prose-h4:mt-4 prose-h4:mb-2 prose-a:text-stone-700 prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-neutral-200 prose-code:rounded prose-code:px-1.5 prose-code:py-0.5 prose-code:text-sm prose-code:font-mono prose-code:text-stone-700 prose-pre:bg-stone-50 prose-pre:border prose-pre:border-neutral-200 prose-pre:rounded-xs prose-pre:prose-code:bg-transparent prose-pre:prose-code:border-0 prose-pre:prose-code:p-0 prose-img:rounded-lg prose-img:border prose-img:border-neutral-200 prose-img:my-6 mt-12 max-w-none">
            <MDXContent
              code={changelog.mdx}
              components={defaultMDXComponents}
            />
          </article>
        </div>

        {diffUrl && (
          <>
            <div className="border-t border-neutral-100" />
            <div className="mx-auto flex max-w-3xl flex-col items-center px-6 py-16 text-center">
              <h2 className="mb-2 font-serif text-3xl text-stone-700">
                View the Code
              </h2>
              <p className="mb-6 text-neutral-600">
                Curious about what changed? See the full diff on GitHub.
              </p>
              <a
                href={diffUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex h-12 items-center gap-2 rounded-full bg-linear-to-t from-neutral-800 to-neutral-700 px-6 text-base font-medium text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]"
              >
                <Icon icon="mdi:github" className="text-xl" />
                View Diff on GitHub
              </a>
            </div>
          </>
        )}

        <div className="border-t border-neutral-100" />

        <div className="mx-auto max-w-3xl px-6 py-16">
          <RelatedReleases
            currentSlug={changelog.slug}
            allChangelogs={allChangelogs}
          />
        </div>
      </div>
    </main>
  );
}

function DownloadLinksHero({ version }: { version: string }) {
  const links = getDownloadLinks(version);
  const grouped = groupDownloadLinks(links);

  return (
    <div className="flex items-start gap-8">
      <div className="flex flex-col items-center gap-2">
        <h3 className="flex items-center gap-1.5 text-xs font-medium tracking-wider text-stone-500 uppercase">
          <Icon icon="simple-icons:apple" className="text-sm" />
          macOS
        </h3>
        <div className="flex flex-col gap-1.5">
          {grouped.macos.map((link) => (
            <a
              key={link.url}
              href={link.url}
              className={cn([
                "flex h-8 items-center justify-center gap-2 rounded-full px-4 text-sm transition-all",
                "bg-linear-to-b from-white to-stone-50 text-neutral-700",
                "border border-neutral-300",
                "hover:scale-[102%] hover:shadow-xs active:scale-[98%]",
              ])}
            >
              <Download className="size-3.5 shrink-0" />
              <span>{link.label}</span>
            </a>
          ))}
        </div>
      </div>

      <div className="flex flex-col items-center gap-2">
        <h3 className="flex items-center gap-1.5 text-xs font-medium tracking-wider text-stone-500 uppercase">
          <Icon icon="simple-icons:linux" className="text-sm" />
          Linux
        </h3>
        <div className="flex flex-col gap-1.5">
          {grouped.linux.map((link) => (
            <a
              key={link.url}
              href={link.url}
              className={cn([
                "flex h-8 items-center justify-center gap-2 rounded-full px-4 text-sm transition-all",
                "bg-linear-to-b from-white to-stone-50 text-neutral-700",
                "border border-neutral-300",
                "hover:scale-[102%] hover:shadow-xs active:scale-[98%]",
              ])}
            >
              <Download className="size-3.5 shrink-0" />
              <span>{link.label}</span>
            </a>
          ))}
        </div>
      </div>
    </div>
  );
}

function DownloadLinksHeroMobile({ version }: { version: string }) {
  const links = getDownloadLinks(version);
  const grouped = groupDownloadLinks(links);

  const allLinks = [...grouped.macos, ...grouped.linux];

  return (
    <div className="scrollbar-hide -mx-6 overflow-x-auto px-6">
      <div className="mx-auto flex w-max gap-3">
        {allLinks.map((link) => (
          <a
            key={link.url}
            href={link.url}
            className={cn([
              "flex w-36 flex-col items-center gap-2 rounded-2xl px-6 py-4 transition-all",
              "bg-linear-to-b from-white to-stone-50 text-neutral-700",
              "border border-neutral-300",
              "hover:shadow-xs active:scale-[98%]",
            ])}
          >
            <Download className="size-5 shrink-0" />
            <div className="text-center">
              <div className="mb-1 text-xs font-medium tracking-wider text-stone-500 uppercase">
                {link.platform}
              </div>
              <div className="text-sm font-medium">{link.label}</div>
            </div>
          </a>
        ))}
      </div>
    </div>
  );
}

function RelatedReleases({
  currentSlug,
  allChangelogs,
}: {
  currentSlug: string;
  allChangelogs: ChangelogWithMeta[];
}) {
  const currentIndex = allChangelogs.findIndex((c) => c.slug === currentSlug);
  if (currentIndex === -1) return null;

  const total = allChangelogs.length;
  let startIndex: number;
  let endIndex: number;

  if (total <= 5) {
    startIndex = 0;
    endIndex = total;
  } else if (currentIndex <= 2) {
    startIndex = 0;
    endIndex = 5;
  } else if (currentIndex >= total - 2) {
    startIndex = total - 5;
    endIndex = total;
  } else {
    startIndex = currentIndex - 2;
    endIndex = currentIndex + 3;
  }

  const relatedChangelogs = allChangelogs.slice(startIndex, endIndex);

  return (
    <section>
      <div className="mb-8 text-center">
        <h2 className="mb-2 font-serif text-3xl text-stone-700">
          Other Releases
        </h2>
        <p className="text-neutral-600">Explore more versions of Char</p>
      </div>

      <div className="flex gap-4 overflow-x-auto sm:grid sm:grid-cols-5 sm:overflow-visible">
        {relatedChangelogs.map((release) => {
          const version = semver.parse(release.version);
          const isPrerelease = version && version.prerelease.length > 0;
          const nightlyNumber =
            isPrerelease && version?.prerelease[0] === "nightly"
              ? version.prerelease[1]
              : null;
          const isCurrent = release.slug === currentSlug;

          return (
            <Link
              key={release.slug}
              to="/changelog/$slug/"
              params={{ slug: release.slug }}
              className={cn([
                "group block",
                isCurrent && "pointer-events-none",
              ])}
            >
              <article
                className={cn([
                  "flex shrink-0 flex-col items-center gap-2 rounded-lg p-4 transition-all duration-300",
                  isCurrent ? "bg-stone-100" : "hover:bg-stone-50",
                ])}
              >
                <img
                  src={
                    isPrerelease
                      ? "/api/images/icons/nightly-icon.png"
                      : "/api/images/icons/stable-icon.png"
                  }
                  alt="Char"
                  className={cn([
                    "size-12 rounded-xl transition-all duration-300",
                    !isCurrent && "group-hover:scale-110",
                  ])}
                />

                <div className="flex items-center gap-1.5">
                  <h3
                    className={cn([
                      "font-mono text-sm font-medium text-stone-700 transition-colors",
                      !isCurrent && "group-hover:text-stone-800",
                    ])}
                  >
                    {version
                      ? `${version.major}.${version.minor}.${version.patch}`
                      : release.version}
                  </h3>
                  {nightlyNumber !== null && (
                    <span className="inline-flex items-center rounded-full bg-stone-200 px-1.5 py-0.5 text-xs font-medium text-stone-700">
                      #{nightlyNumber}
                    </span>
                  )}
                </div>
              </article>
            </Link>
          );
        })}
      </div>

      <div className="mt-8 text-center">
        <Link
          to="/changelog/"
          className="inline-flex h-12 items-center gap-2 rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-6 text-base font-medium text-neutral-700 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]"
        >
          View all releases
          <Icon icon="mdi:arrow-right" className="text-base" />
        </Link>
      </div>
    </section>
  );
}
