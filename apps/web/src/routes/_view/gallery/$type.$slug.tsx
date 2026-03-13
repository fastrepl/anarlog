import { MDXContent } from "@content-collections/mdx/react";
import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { allShortcuts, allTemplates } from "content-collections";

import { cn } from "@hypr/utils";

import { DownloadButton } from "@/components/download-button";
import { MDXLink } from "@/components/mdx";

type GalleryType = "template" | "shortcut";

export const Route = createFileRoute("/_view/gallery/$type/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const { type, slug } = params;

    if (type !== "template" && type !== "shortcut") {
      throw notFound();
    }

    if (type === "template") {
      const template = allTemplates.find((t) => t.slug === slug);
      if (!template) {
        throw notFound();
      }
      return { type: "template" as const, item: template };
    } else {
      const shortcut = allShortcuts.find((s) => s.slug === slug);
      if (!shortcut) {
        throw notFound();
      }
      return { type: "shortcut" as const, item: shortcut };
    }
  },
  head: ({ loaderData }) => {
    if (!loaderData) return { meta: [] };

    const { type, item } = loaderData;
    const typeLabel = type === "template" ? "Template" : "Shortcut";
    const url = `https://char.com/gallery/${type}/${item.slug}`;

    const ogType = type === "template" ? "templates" : "shortcuts";
    const ogImageUrl = `https://char.com/og?type=${ogType}&title=${encodeURIComponent(item.title)}&category=${encodeURIComponent(item.category)}${item.description ? `&description=${encodeURIComponent(item.description)}` : ""}&v=1`;

    return {
      meta: [
        { title: `${item.title} - ${typeLabel} - Char` },
        { name: "description", content: item.description },
        {
          property: "og:title",
          content: `${item.title} - ${typeLabel}`,
        },
        { property: "og:description", content: item.description },
        { property: "og:type", content: "article" },
        { property: "og:url", content: url },
        { property: "og:image", content: ogImageUrl },
        { name: "twitter:card", content: "summary_large_image" },
        {
          name: "twitter:title",
          content: `${item.title} - ${typeLabel}`,
        },
        { name: "twitter:description", content: item.description },
        { name: "twitter:image", content: ogImageUrl },
      ],
    };
  },
});

function Component() {
  const data = Route.useLoaderData();
  const { type, item } = data;

  return (
    <div className="min-h-screen">
      <div className="mx-auto min-h-screen max-w-6xl border-x border-neutral-100 bg-white">
        <div className="flex">
          <LeftSidebar type={type} item={item} />
          <MainContent type={type} item={item} />
          <RightSidebar type={type} item={item} />
        </div>
      </div>
    </div>
  );
}

function LeftSidebar({
  type,
  item,
}: {
  type: GalleryType;
  item: (typeof allTemplates)[0] | (typeof allShortcuts)[0];
}) {
  return (
    <aside className="hidden w-64 shrink-0 lg:block">
      <div className="scrollbar-hide sticky top-17.25 max-h-[calc(100vh-69px)] overflow-y-auto px-4 pt-6 pb-18">
        <Link
          to="/gallery/"
          search={{ type }}
          className="mb-6 inline-flex items-center gap-2 font-serif text-sm text-neutral-600 transition-colors hover:text-stone-700"
        >
          <Icon icon="mdi:arrow-left" className="text-base" />
          <span>Back to gallery</span>
        </Link>

        <div className="flex flex-col gap-6">
          <div>
            <h3 className="mb-3 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Type
            </h3>
            <span
              className={cn([
                "rounded-full px-2 py-0.5 text-xs font-medium",
                type === "template"
                  ? "bg-blue-50 text-blue-600"
                  : "bg-purple-50 text-purple-600",
              ])}
            >
              {type === "template" ? "Template" : "Shortcut"}
            </span>
          </div>

          <div>
            <h3 className="mb-3 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Category
            </h3>
            <Link
              to="/gallery/"
              search={{ type, category: item.category }}
              className="text-sm text-stone-700 transition-colors hover:text-stone-800"
            >
              {item.category}
            </Link>
          </div>

          <div>
            <h3 className="mb-3 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              On this page
            </h3>
            <nav className="flex flex-col gap-1">
              <a
                href="#content"
                className="block py-1 text-sm text-neutral-500 transition-colors hover:text-stone-700"
              >
                {type === "template" ? "Structure" : "Details"}
              </a>
            </nav>
          </div>
        </div>
      </div>
    </aside>
  );
}

function MainContent({
  type,
  item,
}: {
  type: GalleryType;
  item: (typeof allTemplates)[0] | (typeof allShortcuts)[0];
}) {
  return (
    <main className="min-w-0 flex-1 px-4 py-6 pb-6 lg:px-8">
      <div className="mb-6 lg:hidden">
        <Link
          to="/gallery/"
          search={{ type }}
          className="inline-flex items-center gap-2 font-serif text-sm text-neutral-600 transition-colors hover:text-stone-700"
        >
          <Icon icon="mdi:arrow-left" className="text-base" />
          <span>Back to gallery</span>
        </Link>
      </div>

      <ItemHeader type={type} item={item} />
      <ItemContent type={type} item={item} />
      <SuggestedItems type={type} item={item} />
      <ItemFooter type={type} />
    </main>
  );
}

function ItemHeader({
  type,
  item,
}: {
  type: GalleryType;
  item: (typeof allTemplates)[0] | (typeof allShortcuts)[0];
}) {
  const isTemplate = type === "template";

  return (
    <header id="overview" className="mb-8 scroll-mt-20 lg:mb-12">
      <div className="mb-4 flex items-center gap-2 lg:hidden">
        <span
          className={cn([
            "rounded-full px-2 py-0.5 text-xs font-medium",
            isTemplate
              ? "bg-blue-50 text-blue-600"
              : "bg-purple-50 text-purple-600",
          ])}
        >
          {isTemplate ? "Template" : "Shortcut"}
        </span>
        <span className="text-sm text-neutral-500">{item.category}</span>
      </div>
      <h1 className="mb-4 font-serif text-2xl text-stone-700 sm:text-3xl lg:text-4xl">
        {item.title}
      </h1>
      <p className="mb-6 text-lg leading-relaxed text-neutral-600 lg:text-xl">
        {item.description}
      </p>

      {isTemplate && "targets" in item && item.targets && (
        <div className="flex flex-wrap gap-2">
          {item.targets.map((target) => (
            <span
              key={target}
              className="rounded-full border border-stone-100 bg-stone-50 px-3 py-1 text-sm text-stone-700"
            >
              {target}
            </span>
          ))}
        </div>
      )}
    </header>
  );
}

function ItemContent({
  type,
  item,
}: {
  type: GalleryType;
  item: (typeof allTemplates)[0] | (typeof allShortcuts)[0];
}) {
  const isTemplate = type === "template";

  return (
    <section id="content" className="scroll-mt-20">
      <h2 className="mb-4 font-serif text-xl text-stone-700">
        {isTemplate ? "Structure" : "Details"}
      </h2>
      <div className="rounded-xs border border-neutral-200 bg-white px-6 pt-3 pb-6 lg:px-8 lg:pt-4 lg:pb-8">
        {isTemplate && "sections" in item && (
          <div className="mb-6">
            <h3 className="mb-3 text-sm font-semibold tracking-wider text-stone-700 uppercase">
              Template Sections
            </h3>
            <div className="flex flex-col gap-3">
              {item.sections.map((section, index) => (
                <div
                  key={section.title}
                  className="rounded-lg border border-stone-100 bg-stone-50 p-3"
                >
                  <div className="mb-1 flex items-center gap-3">
                    <span className="flex h-5 w-5 items-center justify-center rounded-full bg-stone-200 text-xs font-medium text-stone-700">
                      {index + 1}
                    </span>
                    <h4 className="text-sm font-medium text-stone-700">
                      {section.title}
                    </h4>
                  </div>
                  <p className="ml-8 text-xs text-neutral-600">
                    {section.description}
                  </p>
                </div>
              ))}
            </div>
          </div>
        )}

        <div className="prose prose-stone prose-headings:font-mono prose-headings:font-semibold prose-h2:text-lg prose-h2:mt-6 prose-h2:mb-3 prose-p:text-neutral-600 max-w-none">
          <MDXContent code={item.mdx} components={{ a: MDXLink }} />
        </div>
      </div>
    </section>
  );
}

function SuggestedItems({
  type,
  item,
}: {
  type: GalleryType;
  item: (typeof allTemplates)[0] | (typeof allShortcuts)[0];
}) {
  const suggestedItems =
    type === "template"
      ? allTemplates.filter(
          (t) => t.category === item.category && t.slug !== item.slug,
        )
      : allShortcuts.filter(
          (s) => s.category === item.category && s.slug !== item.slug,
        );

  if (suggestedItems.length === 0) return null;

  return (
    <section className="mt-12">
      <h2 className="mb-4 font-serif text-xl text-stone-700">
        Other {item.category} {type === "template" ? "templates" : "shortcuts"}
      </h2>
      <div className="grid gap-4">
        {suggestedItems.map((t) => (
          <Link
            key={t.slug}
            to="/gallery/$type/$slug/"
            params={{ type, slug: t.slug }}
            className="group rounded-xs border border-neutral-200 bg-white p-4 transition-all hover:border-neutral-300 hover:shadow-md"
          >
            <h3 className="mb-1 font-serif text-lg text-stone-700 transition-colors group-hover:text-stone-800">
              {t.title}
            </h3>
            <p className="line-clamp-2 text-sm text-neutral-600">
              {t.description}
            </p>
          </Link>
        ))}
      </div>
    </section>
  );
}

function ItemFooter({ type }: { type: GalleryType }) {
  return (
    <footer className="mt-12 border-t border-neutral-100 pt-8">
      <Link
        to="/gallery/"
        search={{ type }}
        className="inline-flex items-center gap-2 font-medium text-neutral-600 transition-colors hover:text-stone-700"
      >
        <span>&larr;</span>
        <span>View all {type === "template" ? "templates" : "shortcuts"}</span>
      </Link>
    </footer>
  );
}

function RightSidebar({
  type,
  item,
}: {
  type: GalleryType;
  item: (typeof allTemplates)[0] | (typeof allShortcuts)[0];
}) {
  const isTemplate = type === "template";
  const contentDir = isTemplate ? "templates" : "shortcuts";
  const rawMdxUrl = `https://github.com/fastrepl/char/blob/main/apps/web/content/${contentDir}/${item.slug}.mdx?plain=1`;

  return (
    <aside className="hidden w-80 shrink-0 sm:block">
      <div className="sticky top-17.25 flex flex-col gap-4 px-4 py-6">
        <div className="overflow-hidden rounded-xs border border-neutral-200 bg-white bg-[linear-gradient(to_right,#f5f5f5_1px,transparent_1px),linear-gradient(to_bottom,#f5f5f5_1px,transparent_1px)] bg-size-[24px_24px] bg-position-[12px_12px,12px_12px] p-6 text-center">
          <h3 className="mb-3 font-serif text-lg text-stone-700">
            Use this {isTemplate ? "template" : "shortcut"}
          </h3>
          <p className="mb-6 text-sm text-neutral-600">
            Download Char to use this {isTemplate ? "template" : "shortcut"} and
            get AI-powered meeting notes.
          </p>
          <DownloadButton />
          <p className="mt-4 text-xs text-neutral-500">
            Free to use. No credit card required.
          </p>
        </div>

        <a
          href={rawMdxUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center justify-center gap-2 rounded-xs border border-neutral-200 bg-white px-4 py-3 text-sm text-neutral-600 transition-colors hover:bg-neutral-50 hover:text-neutral-800"
        >
          <Icon icon="mdi:file-document-outline" className="text-lg" />
          View raw MDX source
        </a>

        <div className="rounded-xs border border-dashed border-neutral-300 bg-stone-50/50 p-6 text-center">
          <h3 className="mb-3 font-serif text-lg text-stone-700">Contribute</h3>
          <p className="mb-6 text-sm text-neutral-600">
            Have an idea? Submit a PR and help the community.
          </p>
          <a
            href={`https://github.com/fastrepl/char/tree/main/apps/web/content/${contentDir}`}
            target="_blank"
            rel="noopener noreferrer"
            className={cn([
              "group inline-flex h-12 w-fit items-center justify-center gap-2 px-6",
              "rounded-full bg-linear-to-t from-neutral-800 to-neutral-700 text-white",
              "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
              "cursor-pointer text-base transition-all sm:text-lg",
            ])}
          >
            <Icon icon="mdi:github" className="text-xl" />
            Open on GitHub
          </a>
        </div>
      </div>
    </aside>
  );
}
