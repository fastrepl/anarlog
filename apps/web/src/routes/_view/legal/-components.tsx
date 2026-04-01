import { MDXContent } from "@content-collections/mdx/react";
import { Link, useMatchRoute } from "@tanstack/react-router";
import { motion } from "motion/react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { defaultMDXComponents } from "@/components/mdx";
import { TableOfContents } from "@/components/table-of-contents";

import { getLegalSections, getOrderedLegals } from "./-structure";

const orderedLegals = getOrderedLegals();
const legalSections = getLegalSections();

export function LegalLayout({ doc }: { doc: any }) {
  return (
    <LegalPageShell toc={doc.toc}>
      <main className="mx-auto max-w-200 px-4 py-6">
        <ArticleHeader doc={doc} />
        <ArticleContent doc={doc} />
        <PageNavigation currentSlug={doc.slug} />
      </main>
    </LegalPageShell>
  );
}

export function LegalIndexLayout() {
  return (
    <LegalPageShell>
      <main className="mx-auto max-w-200 px-4 py-6">
        <header className="mb-8 lg:mb-12">
          <div className="mb-4 inline-flex items-center gap-2 text-sm text-neutral-500">
            <span>Legal</span>
          </div>
          <h1 className="mb-4 font-serif text-3xl text-stone-700 sm:text-4xl">
            Legal
          </h1>
          <p className="max-w-3xl text-lg leading-relaxed text-neutral-600 lg:text-xl">
            Terms, privacy policy, and related legal documents for Char.
          </p>
        </header>

        <div className="overflow-hidden rounded-xs border border-neutral-200 bg-white">
          {orderedLegals.map((doc, index) => (
            <LegalDocumentRow
              key={doc.slug}
              doc={doc}
              isLast={index === orderedLegals.length - 1}
            />
          ))}
        </div>
      </main>
    </LegalPageShell>
  );
}

function LegalPageShell({
  children,
  toc = [],
}: {
  children: React.ReactNode;
  toc?: Array<{ id: string; text: string; level: number }>;
}) {
  return (
    <div
      className="min-h-[calc(100vh-4rem)] bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <LeftSidebar />
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        {children}
      </div>
      <TableOfContents toc={toc} />
    </div>
  );
}

function LeftSidebar() {
  const [isOpen, setIsOpen] = useState(
    () => typeof window !== "undefined" && window.innerWidth > 1400,
  );
  const closeTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const activeLinkRef = useRef<HTMLAnchorElement>(null);

  useEffect(() => {
    const mq = window.matchMedia("(min-width: 1400px)");
    const handler = (e: MediaQueryListEvent) => {
      if (e.matches) setIsOpen(true);
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  const matchRoute = useMatchRoute();
  const match = matchRoute({ to: "/legal/$slug/", fuzzy: true });
  const currentSlug =
    match && typeof match !== "boolean" ? (match.slug as string) : undefined;

  useEffect(() => {
    if (!activeLinkRef.current || !scrollContainerRef.current) {
      return;
    }

    requestAnimationFrame(() => {
      const container = scrollContainerRef.current;
      const activeLink = activeLinkRef.current;

      if (!container || !activeLink) {
        return;
      }

      const containerRect = container.getBoundingClientRect();
      const linkRect = activeLink.getBoundingClientRect();

      container.scrollTop =
        activeLink.offsetTop -
        container.offsetTop -
        containerRect.height / 2 +
        linkRect.height / 2;
    });
  }, [currentSlug]);

  useEffect(() => {
    return () => {
      if (closeTimeoutRef.current) {
        clearTimeout(closeTimeoutRef.current);
      }
    };
  }, []);

  const open = () => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current);
      closeTimeoutRef.current = null;
    }
    setIsOpen(true);
  };

  const close = () => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current);
    }
    if (window.innerWidth > 1400) return;
    closeTimeoutRef.current = setTimeout(() => {
      setIsOpen(false);
    }, 300);
  };

  return (
    <motion.div
      className="fixed top-1/2 left-0 z-30 hidden h-[80vh] -translate-y-1/2 drop-shadow-lg md:flex"
      initial={false}
      animate={{ x: isOpen ? 0 : -256 }}
      transition={{ type: "spring", stiffness: 400, damping: 35 }}
      onMouseEnter={open}
      onMouseLeave={close}
    >
      <div className="h-full w-64 overflow-hidden rounded-r-2xl border border-l-0 border-neutral-200 bg-white/95 backdrop-blur-sm">
        <div
          ref={scrollContainerRef}
          className="scrollbar-hide h-full overflow-y-auto px-4 py-6"
        >
          <nav className="flex flex-col gap-4">
            {legalSections.map((section) => (
              <div key={section.title}>
                <h3 className="mb-2 px-3 text-sm font-semibold text-neutral-700">
                  {section.title}
                </h3>
                <div className="flex flex-col gap-0.5">
                  {section.docs.map((doc) => {
                    const isActive = currentSlug === doc.slug;

                    return (
                      <Link
                        key={doc.slug}
                        to="/legal/$slug/"
                        params={{ slug: doc.slug }}
                        onClick={() => {
                          if (window.innerWidth <= 1400) {
                            setIsOpen(false);
                          }
                        }}
                        ref={isActive ? activeLinkRef : undefined}
                        className={cn([
                          "block rounded-xs py-1.5 pr-3 pl-5 text-sm transition-colors",
                          isActive
                            ? "bg-neutral-100 font-medium text-stone-600"
                            : "text-neutral-600 hover:bg-neutral-50 hover:text-stone-600",
                        ])}
                      >
                        {doc.title}
                      </Link>
                    );
                  })}
                </div>
              </div>
            ))}
          </nav>
        </div>
      </div>
      <motion.div
        className="-ml-px self-center"
        animate={{ opacity: isOpen ? 0 : 1 }}
        transition={{ duration: 0.2 }}
      >
        <div
          className={cn([
            "flex items-center justify-center",
            "h-20 w-7 rounded-r-xl",
            "border border-l-0 border-neutral-200 bg-white/95 backdrop-blur-sm",
            "cursor-pointer text-neutral-400 transition-colors hover:text-neutral-600",
          ])}
        >
          <motion.svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            animate={{ rotate: isOpen ? 180 : 0 }}
            transition={{ duration: 0.2 }}
          >
            <path
              d="M5 3L9.5 7L5 11"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </motion.svg>
        </div>
      </motion.div>
    </motion.div>
  );
}

function ArticleHeader({ doc }: { doc: any }) {
  return (
    <header className="mb-8 lg:mb-12">
      <div className="mb-4 inline-flex items-center gap-2 text-sm text-neutral-500">
        <span>Legal</span>
      </div>
      <h1 className="mb-4 font-serif text-3xl text-stone-700 sm:text-4xl">
        {doc.title}
      </h1>
      {doc.summary && (
        <p className="mb-6 max-w-3xl text-lg leading-relaxed text-neutral-600 lg:text-xl">
          {doc.summary}
        </p>
      )}

      <div className="flex items-center gap-4 text-sm text-neutral-500">
        <time dateTime={doc.date}>
          Updated{" "}
          {new Date(doc.date).toLocaleDateString("en-US", {
            year: "numeric",
            month: "long",
            day: "numeric",
          })}
        </time>
      </div>
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

function LegalDocumentRow({ doc, isLast }: { doc: any; isLast: boolean }) {
  return (
    <Link
      to="/legal/$slug/"
      params={{ slug: doc.slug }}
      className={cn([
        "group block px-5 py-5 transition-colors sm:px-6",
        !isLast && "border-b border-neutral-100",
        "hover:bg-stone-50/70",
      ])}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <h2 className="font-serif text-xl text-stone-700 transition-colors group-hover:text-stone-900">
            {doc.title}
          </h2>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-neutral-600 sm:text-base">
            {doc.summary}
          </p>
        </div>

        <span className="shrink-0 text-sm text-neutral-400 transition-colors group-hover:text-stone-600">
          Read
        </span>
      </div>

      <p className="mt-4 text-sm text-neutral-500">
        Updated{" "}
        {new Date(doc.date).toLocaleDateString("en-US", {
          month: "long",
          day: "numeric",
          year: "numeric",
        })}
      </p>
    </Link>
  );
}

function PageNavigation({ currentSlug }: { currentSlug: string }) {
  const currentIndex = orderedLegals.findIndex(
    (doc) => doc.slug === currentSlug,
  );
  const prev = currentIndex > 0 ? orderedLegals[currentIndex - 1] : null;
  const next =
    currentIndex >= 0 && currentIndex < orderedLegals.length - 1
      ? orderedLegals[currentIndex + 1]
      : null;

  if (!prev && !next) return null;

  return (
    <nav className="mt-12 flex items-center justify-between gap-4 border-t border-neutral-200 pt-6">
      {prev ? (
        <Link
          to="/legal/$slug/"
          params={{ slug: prev.slug }}
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
          to="/legal/$slug/"
          params={{ slug: next.slug }}
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
