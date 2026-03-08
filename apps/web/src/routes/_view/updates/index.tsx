import { MDXContent } from "@content-collections/mdx/react";
import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { allUpdates, type Update } from "content-collections";

import { EmailSubscribeField } from "@/components/email-subscribe-field";
import { defaultMDXComponents } from "@/components/mdx";

function getWeekLabel(dateStr: string): string {
  const d = new Date(dateStr + "T00:00:00");
  const year = d.getFullYear();
  const jan1 = new Date(year, 0, 1);
  const days = Math.floor((d.getTime() - jan1.getTime()) / 86400000);
  const week = Math.ceil((days + jan1.getDay() + 1) / 7);
  return `Week ${week} ${year}`;
}

export const Route = createFileRoute("/_view/updates/")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Updates - Char" },
      {
        name: "description",
        content: "Weekly updates from the Char team",
      },
      { property: "og:title", content: "Updates - Char" },
      {
        property: "og:description",
        content: "Weekly updates from the Char team",
      },
      { property: "og:type", content: "website" },
      { property: "og:url", content: "https://char.com/updates" },
    ],
  }),
});

function Component() {
  const sortedUpdates = [...allUpdates].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
  );
  return (
    <main
      className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <div className="px-6 py-16 lg:py-24">
          <HeroSection />
        </div>
        <div className="border-t border-neutral-100" />
        <div className="py-8">
          {sortedUpdates.map((update, index) => (
            <div key={update.slug}>
              <div className="mx-auto max-w-4xl px-6">
                <UpdateSection update={update} />
              </div>
              {index < sortedUpdates.length - 1 && (
                <div className="my-8 border-b border-neutral-100" />
              )}
            </div>
          ))}
        </div>
        <div className="border-t border-neutral-100" />
        <div className="mx-auto max-w-3xl px-6 py-16 lg:py-24">
          <SubscribeSection />
        </div>
      </div>
    </main>
  );
}

function HeroSection() {
  return (
    <div className="flex flex-col items-center gap-6 text-center">
      <h1 className="font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
        Updates
      </h1>
      <p className="text-lg text-neutral-600 sm:text-xl">
        Weekly updates from the Char team
      </p>
      <EmailSubscribeField
        className="w-full max-w-md"
        formClassName="w-full"
        variant="hero"
      />
    </div>
  );
}

function SubscribeSection() {
  return (
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
  );
}

function UpdateSection({ update }: { update: Update }) {
  return (
    <section className="grid grid-cols-1 gap-6 md:grid-cols-[160px_1fr] md:gap-12">
      <div className="flex flex-col gap-1 md:sticky md:top-24 md:self-start">
        <Link to="/updates/$slug/" params={{ slug: update.slug }}>
          <h2 className="cursor-pointer font-serif text-xl font-medium text-stone-700 transition-colors hover:text-stone-900">
            {getWeekLabel(update.date)}
          </h2>
        </Link>
        <time className="mt-1 text-sm text-neutral-500" dateTime={update.date}>
          {new Date(update.date).toLocaleDateString("en-US", {
            year: "numeric",
            month: "long",
            day: "numeric",
          })}
        </time>
      </div>

      <div className="min-w-0">
        <div className="relative h-[20rem] overflow-hidden">
          <article className="prose prose-stone prose-sm prose-headings:font-serif prose-headings:font-semibold prose-h2:text-lg prose-h2:mt-4 prose-h2:mb-2 prose-h3:text-base prose-h3:mt-3 prose-h3:mb-1 prose-ul:my-2 prose-li:my-0.5 prose-a:text-stone-600 prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-neutral-200 prose-code:rounded prose-code:px-1 prose-code:py-0.5 prose-code:text-xs prose-code:font-mono prose-code:text-stone-700 prose-img:rounded prose-img:border prose-img:border-neutral-200 prose-img:my-3 max-w-none">
            <MDXContent code={update.mdx} components={defaultMDXComponents} />
          </article>
          <div className="pointer-events-none absolute inset-x-0 bottom-0 h-20 bg-linear-to-t from-white via-white/90 to-transparent" />
          <Link
            to="/updates/$slug/"
            params={{ slug: update.slug }}
            className="absolute bottom-1 left-0 z-10 inline-flex items-center gap-1 bg-white/95 pr-2 text-sm text-stone-600 transition-colors hover:text-stone-900"
          >
            Read more
            <Icon icon="mdi:arrow-right" className="text-base" />
          </Link>
        </div>
      </div>
    </section>
  );
}
