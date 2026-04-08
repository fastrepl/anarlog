import { MDXContent } from "@content-collections/mdx/react";
import { Link } from "@tanstack/react-router";
import { allDocs } from "content-collections";
import { useMemo } from "react";

import { AcquisitionLinkGrid } from "@/components/acquisition-link-grid";
import { defaultMDXComponents } from "@/components/mdx";
import { TableOfContents } from "@/components/table-of-contents";

import { docsStructure } from "./-structure";

export function DocLayout({
  doc,
  showSectionTitle = true,
}: {
  doc: any;
  showSectionTitle?: boolean;
}) {
  return (
    <>
      <main className="max-w-200 px-16 py-6">
        <ArticleHeader doc={doc} showSectionTitle={showSectionTitle} />
        <ArticleContent doc={doc} />
        <PageNavigation currentSlug={doc.slug} />
        <DocExploreSection doc={doc} />
      </main>
      <TableOfContents toc={doc.toc} />
    </>
  );
}

function ArticleHeader({
  doc,
  showSectionTitle,
}: {
  doc: any;
  showSectionTitle: boolean;
}) {
  const sectionTitle =
    allDocs.find((d) => d.sectionFolder === doc.sectionFolder && d.isIndex)
      ?.title ||
    doc.sectionFolder
      .split("-")
      .map((word: string) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");

  return (
    <header className="mb-8 lg:mb-12">
      {showSectionTitle && (
        <div className="mb-4 inline-flex items-center gap-2 text-sm text-neutral-500">
          <span>{sectionTitle}</span>
        </div>
      )}
      <h1 className="mb-4 font-serif text-3xl text-stone-700 sm:text-4xl">
        {doc.title}
      </h1>
      {doc.summary && (
        <p className="mb-6 text-lg leading-relaxed text-neutral-600 lg:text-xl">
          {doc.summary}
        </p>
      )}

      {(doc.author || doc.created) && (
        <div className="flex items-center gap-4 text-sm text-neutral-500">
          {doc.author && <span>{doc.author}</span>}
          {doc.author && doc.created && <span>·</span>}
          {doc.created && (
            <time dateTime={doc.created}>
              {new Date(doc.created).toLocaleDateString("en-US", {
                year: "numeric",
                month: "long",
                day: "numeric",
              })}
            </time>
          )}
          {doc.updated && doc.updated !== doc.created && (
            <>
              <span>·</span>
              <span className="text-neutral-400">
                Updated{" "}
                {new Date(doc.updated).toLocaleDateString("en-US", {
                  year: "numeric",
                  month: "long",
                  day: "numeric",
                })}
              </span>
            </>
          )}
        </div>
      )}
    </header>
  );
}

function ArticleContent({ doc }: { doc: any }) {
  return (
    <article className="prose prose-stone prose-headings:font-serif prose-headings:font-semibold prose-h1:text-3xl prose-h1:mt-12 prose-h1:mb-6 prose-h2:text-2xl prose-h2:mt-10 prose-h2:mb-5 prose-h3:text-xl prose-h3:mt-8 prose-h3:mb-4 prose-h4:text-lg prose-h4:mt-6 prose-h4:mb-3 prose-a:text-stone-600 prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-neutral-200 prose-code:rounded prose-code:px-1.5 prose-code:py-0.5 prose-code:text-sm prose-code:font-mono prose-code:text-stone-700 prose-pre:bg-stone-50 prose-pre:border prose-pre:border-neutral-200 prose-pre:rounded-xs prose-pre:prose-code:bg-transparent prose-pre:prose-code:border-0 prose-pre:prose-code:p-0 prose-img:rounded-xs prose-img:my-8 max-w-none">
      <MDXContent code={doc.mdx} components={defaultMDXComponents} />
    </article>
  );
}

function PageNavigation({ currentSlug }: { currentSlug: string }) {
  const { prev, next } = useMemo(() => {
    const orderedPages = docsStructure.sections.flatMap((sectionId) => {
      return allDocs
        .filter(
          (doc) =>
            doc.section.toLowerCase() === sectionId.toLowerCase() &&
            !doc.isIndex,
        )
        .sort((a, b) => a.order - b.order);
    });

    const currentIndex = orderedPages.findIndex(
      (doc) => doc.slug === currentSlug,
    );

    return {
      prev: currentIndex > 0 ? orderedPages[currentIndex - 1] : null,
      next:
        currentIndex < orderedPages.length - 1
          ? orderedPages[currentIndex + 1]
          : null,
    };
  }, [currentSlug]);

  if (!prev && !next) return null;

  return (
    <nav className="mt-12 flex items-center justify-between gap-4 border-t border-neutral-200 pt-6">
      {prev ? (
        <Link
          to="/docs/$/"
          params={{ _splat: prev.slug }}
          className="group flex flex-col items-start gap-1 text-sm"
        >
          <span className="text-neutral-400 transition-colors group-hover:text-neutral-500">
            Previous
          </span>
          <span className="font-medium text-stone-600 transition-colors group-hover:text-stone-800">
            {prev.title}
          </span>
        </Link>
      ) : (
        <div />
      )}
      {next ? (
        <Link
          to="/docs/$/"
          params={{ _splat: next.slug }}
          className="group flex flex-col items-end gap-1 text-right text-sm"
        >
          <span className="text-neutral-400 transition-colors group-hover:text-neutral-500">
            Next
          </span>
          <span className="font-medium text-stone-600 transition-colors group-hover:text-stone-800">
            {next.title}
          </span>
        </Link>
      ) : (
        <div />
      )}
    </nav>
  );
}

function DocExploreSection({ doc }: { doc: any }) {
  const items =
    doc.sectionFolder === "developers"
      ? [
          {
            eyebrow: "Solutions",
            title: "Char for developers",
            description:
              "See the landing page for teams that want local-first, open-source meeting notes they can inspect and extend.",
            href: "/solution/engineering",
          },
          {
            eyebrow: "Integrations",
            title: "Browse meeting platform guides",
            description:
              "See how Char works with Zoom, Google Meet, Teams, and Webex without inviting a bot.",
            href: "/integrations/",
          },
          {
            eyebrow: "Comparisons",
            title: "Compare Char vs Otter",
            description:
              "Start with one of the most common evaluation paths for teams leaving cloud-first note takers.",
            href: "/vs/otter",
          },
        ]
      : doc.sectionFolder === "faq" || doc.sectionFolder === "pro"
        ? [
            {
              eyebrow: "Pricing",
              title: "Compare Free, Lite, and Pro",
              description:
                "See what stays local on the free plan and what the managed cloud plans unlock.",
              href: "/pricing",
            },
            {
              eyebrow: "Integrations",
              title: "Meeting platform guides",
              description:
                "Jump from the docs into the landing pages for Zoom, Meet, Teams, and Webex workflows.",
              href: "/integrations/",
            },
            {
              eyebrow: "Solutions",
              title: "Browse team workflows",
              description:
                "Explore the solution pages for sales, research, legal, and other conversation-heavy teams.",
              href: "/solutions/",
            },
          ]
        : [
            {
              eyebrow: "Solutions",
              title: "Browse team workflows",
              description:
                "See the use-case pages for sales, research, legal, coaching, and more.",
              href: "/solutions/",
            },
            {
              eyebrow: "Integrations",
              title: "Browse meeting platform guides",
              description:
                "Find the platform-specific pages for Zoom, Google Meet, Teams, and Webex.",
              href: "/integrations/",
            },
            {
              eyebrow: "Comparisons",
              title: "Compare Char vs Otter",
              description:
                "See a direct evaluation page for one of the most common note-taking alternatives.",
              href: "/vs/otter",
            },
          ];

  return (
    <AcquisitionLinkGrid
      title="Popular next steps"
      description="If you came here from evaluation or implementation work, these are the shortest paths back to the core acquisition pages."
      className="mt-12"
      items={items}
    />
  );
}
