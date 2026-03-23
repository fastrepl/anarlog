import { MDXContent } from "@content-collections/mdx/react";
import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { allRoadmaps } from "content-collections";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";
import { MDXLink, Mermaid, Tweet } from "@/components/mdx";

export const Route = createFileRoute("/_view/roadmap/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const item = allRoadmaps.find((r) => r.slug === params.slug);
    if (!item) {
      throw notFound();
    }
    return { item };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.item) {
      return { meta: [] };
    }

    const { item } = loaderData;
    const url = `https://char.com/roadmap/${item.slug}`;

    return {
      meta: [
        { title: `${item.title} - Roadmap - Char` },
        {
          name: "description",
          content: `Roadmap item: ${item.title}`,
        },
        { property: "og:title", content: item.title },
        {
          property: "og:description",
          content: `Roadmap item: ${item.title}`,
        },
        { property: "og:type", content: "article" },
        { property: "og:url", content: url },
        { name: "twitter:card", content: "summary" },
        { name: "twitter:title", content: item.title },
        {
          name: "twitter:description",
          content: `Roadmap item: ${item.title}`,
        },
      ],
    };
  },
});

function Component() {
  const { item } = Route.useLoaderData();

  const statusConfig = {
    done: {
      label: "Done",
      icon: "mdi:check-circle",
      className: "bg-linear-to-t from-green-200 to-green-100 text-green-900",
    },
    "in-progress": {
      label: "In Progress",
      icon: "mdi:progress-clock",
      className: "bg-linear-to-b from-[#03BCF1] to-[#127FE5] text-white",
    },
    todo: {
      label: "To Do",
      icon: "mdi:calendar-clock",
      className:
        "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900",
    },
  };

  const status = statusConfig[item.status];

  return (
    <div
      className="bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white px-4 py-8 sm:px-6 lg:px-8">
        <Link
          to="/roadmap/"
          className="mb-8 inline-flex items-center gap-2 font-serif text-sm text-neutral-600 transition-colors hover:text-stone-600"
        >
          <Icon icon="mdi:arrow-left" className="text-base" />
          <span>Back to roadmap</span>
        </Link>

        <article>
          <header className="mb-8">
            <h1 className="mb-4 font-serif text-3xl text-stone-700 sm:text-4xl lg:text-5xl">
              {item.title}
            </h1>

            <div className="mb-4 flex flex-wrap items-center gap-2">
              <span
                className={cn([
                  "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium",
                  status.className,
                ])}
              >
                <Icon icon={status.icon} className="text-xs" />
                {status.label}
              </span>

              {item.labels &&
                item.labels.length > 0 &&
                item.labels.map((label) => (
                  <span
                    key={label}
                    className="inline-flex items-center rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-900 capitalize"
                  >
                    {label}
                  </span>
                ))}
            </div>

            <div className="font-mono text-xs text-neutral-500">
              <time dateTime={item.date}>
                {new Date(item.date).toLocaleDateString("en-US", {
                  year: "numeric",
                  month: "short",
                  day: "numeric",
                })}
              </time>
            </div>
          </header>

          <div className="prose prose-stone prose-lg prose-headings:font-serif prose-headings:font-semibold prose-h1:text-3xl prose-h1:mt-12 prose-h1:mb-6 prose-h2:text-2xl prose-h2:mt-10 prose-h2:mb-5 prose-h3:text-xl prose-h3:mt-8 prose-h3:mb-4 prose-h4:text-lg prose-h4:mt-6 prose-h4:mb-3 prose-a:text-stone-600 prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-neutral-200 prose-code:rounded prose-code:px-1.5 prose-code:py-0.5 prose-code:text-sm prose-code:font-mono prose-code:text-stone-700 prose-pre:bg-stone-50 prose-pre:border prose-pre:border-neutral-200 prose-pre:rounded-xs prose-pre:prose-code:bg-transparent prose-pre:prose-code:border-0 prose-pre:prose-code:p-0 prose-img:rounded-xs prose-img:border prose-img:border-neutral-200 prose-img:my-8 max-w-none">
            <MDXContent
              code={item.mdx}
              components={{
                a: MDXLink,
                Image,
                img: Image,
                mermaid: Mermaid,
                Mermaid,
                Tweet,
              }}
            />
          </div>

          <div className="mt-12 border-t border-neutral-100 pt-8">
            <h3 className="mb-6 font-serif text-xl text-stone-700">
              Related GitHub Issues
            </h3>
            {item.githubIssues && item.githubIssues.length > 0 ? (
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                {item.githubIssues.map((url) => (
                  <GitHubIssuePreview key={url} url={url} />
                ))}
              </div>
            ) : (
              <p className="text-sm text-neutral-400">
                No related GitHub issues yet
              </p>
            )}
          </div>
        </article>
      </div>
    </div>
  );
}

function GitHubIssuePreview({ url }: { url: string }) {
  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      className={cn([
        "flex items-center gap-2 rounded-lg border border-neutral-200 bg-white px-4 py-3",
        "text-sm text-stone-600 transition-colors hover:border-neutral-300 hover:text-stone-800",
      ])}
    >
      <Icon icon="mdi:github" className="text-lg" />
      <span className="flex-1">{url}</span>
      <Icon icon="mdi:open-in-new" className="text-xs text-neutral-400" />
    </a>
  );
}
