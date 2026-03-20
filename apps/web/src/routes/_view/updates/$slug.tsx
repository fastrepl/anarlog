import { MDXContent } from "@content-collections/mdx/react";
import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { allUpdates, type Update } from "content-collections";

import { cn } from "@hypr/utils";

import { EmailSubscribeField } from "@/components/email-subscribe-field";
import { defaultMDXComponents } from "@/components/mdx";
import { NotFoundContent } from "@/components/not-found";

function getWeekLabel(dateStr: string): string {
  const d = new Date(dateStr + "T00:00:00");
  const year = d.getFullYear();
  const jan1 = new Date(year, 0, 1);
  const days = Math.floor((d.getTime() - jan1.getTime()) / 86400000);
  const week = Math.ceil((days + jan1.getDay() + 1) / 7);
  return `Week ${week} ${year}`;
}

function getUpdateBySlug(slug: string): Update | undefined {
  return allUpdates.find((u) => u.slug === slug);
}

function getSortedUpdates(): Update[] {
  return [...allUpdates].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
  );
}

export const Route = createFileRoute("/_view/updates/$slug")({
  component: Component,
  notFoundComponent: NotFoundContent,
  loader: async ({ params }) => {
    const update = getUpdateBySlug(params.slug);
    if (!update) {
      throw notFound();
    }

    const sorted = getSortedUpdates();
    const currentIndex = sorted.findIndex((u) => u.slug === params.slug);
    const newerSlug = currentIndex > 0 ? sorted[currentIndex - 1].slug : null;
    const olderSlug =
      currentIndex < sorted.length - 1 ? sorted[currentIndex + 1].slug : null;

    return { update, newerSlug, olderSlug, sorted };
  },
  head: ({ loaderData }) => {
    if (!loaderData) return {};

    const { update } = loaderData;
    const weekLabel = getWeekLabel(update.date);
    const title = `${weekLabel} - Char Updates`;
    const description = `Weekly update from the Char team — ${weekLabel}`;
    const url = `https://char.com/updates/${update.slug}`;

    return {
      meta: [
        { title },
        { name: "description", content: description },
        { property: "og:type", content: "article" },
        { property: "og:title", content: title },
        { property: "og:description", content: description },
        { property: "og:url", content: url },
      ],
    };
  },
});

function Component() {
  const { update, newerSlug, olderSlug, sorted } = Route.useLoaderData();

  return (
    <main
      className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <div className="mx-auto max-w-3xl px-6 pt-16 pb-8 lg:pt-24">
          <div className="mb-12 flex flex-col items-center gap-2 text-center">
            <h1 className="font-serif text-3xl font-medium text-stone-700 sm:text-4xl">
              {getWeekLabel(update.date)}
            </h1>
            <time className="text-sm text-neutral-500" dateTime={update.date}>
              {new Date(update.date).toLocaleDateString("en-US", {
                year: "numeric",
                month: "long",
                day: "numeric",
              })}
            </time>
          </div>

          <article className="prose prose-stone prose-headings:font-serif prose-headings:font-semibold prose-h2:text-2xl prose-h2:mt-8 prose-h2:mb-4 prose-h3:text-xl prose-h3:mt-6 prose-h3:mb-3 prose-h4:text-lg prose-h4:mt-4 prose-h4:mb-2 prose-a:text-stone-700 prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-neutral-200 prose-code:rounded prose-code:px-1.5 prose-code:py-0.5 prose-code:text-sm prose-code:font-mono prose-code:text-stone-700 prose-img:rounded-lg prose-img:border prose-img:border-neutral-200 prose-img:my-6 max-w-none">
            <MDXContent code={update.mdx} components={defaultMDXComponents} />
          </article>
        </div>

        <div className="border-t border-neutral-100" />

        <div className="mx-auto max-w-3xl px-6 py-12">
          <div className="flex items-center justify-center gap-1">
            {newerSlug && (
              <Link
                to="/updates/$slug/"
                params={{ slug: newerSlug }}
                className="inline-flex items-center px-1 py-1 text-stone-400 transition-colors hover:text-stone-700"
              >
                <Icon icon="mdi:arrow-left" className="text-base" />
              </Link>
            )}
            {sorted.map((u) => {
              const label = getWeekLabel(u.date).replace("Week ", "W");
              const isCurrent = u.slug === update.slug;
              return (
                <Link
                  key={u.slug}
                  to="/updates/$slug/"
                  params={{ slug: u.slug }}
                  className={cn([
                    "rounded px-2 py-1 text-sm transition-colors",
                    isCurrent
                      ? "bg-stone-100 font-medium text-stone-900"
                      : "text-stone-500 hover:text-stone-800",
                  ])}
                >
                  {label}
                </Link>
              );
            })}
            {olderSlug && (
              <Link
                to="/updates/$slug/"
                params={{ slug: olderSlug }}
                className="inline-flex items-center px-1 py-1 text-stone-400 transition-colors hover:text-stone-700"
              >
                <Icon icon="mdi:arrow-right" className="text-base" />
              </Link>
            )}
          </div>
        </div>

        <div className="border-t border-neutral-100" />

        <div className="mx-auto max-w-3xl px-6 py-16 lg:py-24">
          <div className="flex flex-col items-center gap-4 text-center">
            <h2 className="font-serif text-3xl text-stone-700">
              Get updates in your inbox
            </h2>
            <p className="text-neutral-600">
              Subscribe to get weekly updates from the Char team.
            </p>
            <EmailSubscribeField
              className="w-full max-w-md"
              formClassName="w-full"
              variant="hero"
            />
          </div>
        </div>
      </div>
    </main>
  );
}
