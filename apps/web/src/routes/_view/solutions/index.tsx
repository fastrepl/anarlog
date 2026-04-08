import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";

import { CTASection } from "@/components/cta-section";
import { sortedSolutions } from "@/lib/solutions";

export const Route = createFileRoute("/_view/solutions/")({
  component: Component,
  head: () => {
    const url = "https://char.com/solutions";

    return {
      meta: [
        { title: "Solutions - Char" },
        {
          name: "description",
          content:
            "Browse how teams across sales, consulting, healthcare, research, and more use Char to capture and act on every conversation.",
        },
        { tag: "link", attrs: { rel: "canonical", href: url } },
        { property: "og:title", content: "Solutions - Char" },
        {
          property: "og:description",
          content:
            "Browse how teams across sales, consulting, healthcare, research, and more use Char to capture and act on every conversation.",
        },
        { property: "og:type", content: "website" },
        { property: "og:url", content: url },
      ],
    };
  },
});

function Component() {
  return (
    <main className="min-h-screen overflow-x-hidden">
      <div className="mx-auto">
        <section className="border-b border-neutral-100 px-4 py-16 lg:py-24">
          <div className="mx-auto max-w-4xl">
            <div className="mb-4 text-xs font-semibold tracking-[0.2em] text-stone-500 uppercase">
              Solutions
            </div>
            <h1 className="text-color mb-4 font-mono text-4xl tracking-tight sm:text-5xl">
              Char for every team that meets
            </h1>
            <p className="text-fg max-w-3xl text-lg leading-8">
              From account teams and consultants to researchers and healthcare
              staff, Char helps you keep the full record, find what matters
              fast, and share the outcome without extra admin.
            </p>
          </div>
        </section>

        <section className="px-4 py-10 lg:py-12">
          <div className="mx-auto grid max-w-6xl gap-4 md:grid-cols-2 xl:grid-cols-3">
            {sortedSolutions.map((solution) => (
              <Link
                key={solution.slug}
                to="/solution/$slug/"
                params={{ slug: solution.slug }}
                className="group rounded-3xl border border-neutral-200 bg-white p-6 transition-colors hover:border-stone-300 hover:bg-stone-50"
              >
                <div className="mb-5 flex items-start gap-4">
                  <div className="flex size-11 shrink-0 items-center justify-center rounded-2xl bg-stone-100 text-stone-700">
                    <Icon icon={solution.icon} className="text-xl" />
                  </div>
                  <div className="min-w-0">
                    <h2 className="text-fg text-lg font-medium">
                      {solution.label}
                    </h2>
                    <p className="text-fg-muted mt-2 text-sm leading-6">
                      {solution.metaDescription}
                    </p>
                  </div>
                </div>

                <div className="mb-5 flex flex-wrap gap-2">
                  {solution.useCases.items.slice(0, 3).map((item) => (
                    <span
                      key={item.title}
                      className="rounded-full bg-stone-100 px-3 py-1 text-xs text-stone-700"
                    >
                      {item.title}
                    </span>
                  ))}
                </div>

                <div className="inline-flex items-center gap-2 text-sm font-medium text-stone-700 transition-colors group-hover:text-stone-950">
                  Explore solution
                  <ChevronRight size={16} />
                </div>
              </Link>
            ))}
          </div>
        </section>

        <CTASection
          title="Need a setup for your team?"
          description="Pick the workflow that matches your meetings, then adapt Char to your process with templates, local AI, and searchable notes."
        />
      </div>
    </main>
  );
}
