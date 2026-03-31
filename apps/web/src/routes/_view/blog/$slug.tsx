import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { allArticles } from "content-collections";
import { motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { CTASection } from "@/components/cta-section";
import { defaultMDXComponents } from "@/components/mdx";
import { useBlogToc } from "@/hooks/use-blog-toc";
import { AUTHOR_AVATARS } from "@/lib/team";

export const Route = createFileRoute("/_view/blog/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const article = allArticles.find((article) => article.slug === params.slug);
    if (!article) {
      throw notFound();
    }

    const relatedArticles = allArticles
      .filter((a) => a.slug !== article.slug)
      .sort((a, b) => {
        const aScore = a.author.some((name: string) =>
          article.author.includes(name),
        )
          ? 1
          : 0;
        const bScore = b.author.some((name: string) =>
          article.author.includes(name),
        )
          ? 1
          : 0;
        if (aScore !== bScore) {
          return bScore - aScore;
        }

        return new Date(b.date).getTime() - new Date(a.date).getTime();
      })
      .slice(0, 3);

    return { article, relatedArticles };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.article) {
      return { meta: [] };
    }

    const { article } = loaderData;
    const url = `https://char.com/blog/${article.slug}`;

    const title = article.title ?? "";
    const metaDescription = article.meta_description ?? "";
    const ogImage =
      article.coverImage ||
      `https://char.com/og?type=blog&title=${encodeURIComponent(title)}${article.author.length > 0 ? `&author=${encodeURIComponent(article.author.join(", "))}` : ""}${article.date ? `&date=${encodeURIComponent(new Date(article.date).toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric" }))}` : ""}&v=1`;

    return {
      meta: [
        { title: `${title} - Char Blog` },
        { name: "description", content: metaDescription },
        { tag: "link", attrs: { rel: "canonical", href: url } },
        {
          property: "og:title",
          content: `${title} - Char Blog`,
        },
        {
          property: "og:description",
          content: metaDescription,
        },
        { property: "og:type", content: "article" },
        { property: "og:url", content: url },
        { property: "og:image", content: ogImage },
        { name: "twitter:card", content: "summary_large_image" },
        {
          name: "twitter:title",
          content: `${title} - Char Blog`,
        },
        {
          name: "twitter:description",
          content: metaDescription,
        },
        { name: "twitter:image", content: ogImage },
        ...(article.author.length > 0
          ? [{ name: "author", content: article.author.join(", ") }]
          : []),
        {
          property: "article:published_time",
          content: article.date,
        },
      ],
    };
  },
});

function Component() {
  const { article, relatedArticles } = Route.useLoaderData();

  return (
    <main data-blog-article className="min-h-screen flex-1">
      <TableOfContents toc={article.toc} />
      <div className="">
        <HeroSection article={article} />
        <div className="px-4 py-8">
          <ArticleContent article={article} />
          <RelatedArticlesSection relatedArticles={relatedArticles} />
        </div>
        <CTASection />
      </div>
    </main>
  );
}

function HeroSection({ article }: { article: any }) {
  return (
    <header className="px-4 pt-12 pb-8 text-left md:px-8 md:pt-12">
      <Link
        to="/blog/"
        className="text-fg mb-8 inline-flex items-center gap-2 text-sm opacity-50 transition-opacity hover:opacity-100"
      >
        <span>←</span>
        <span>Back to Blog</span>
      </Link>

      {article.category && (
        <p className="text-fg mb-4 font-mono text-sm opacity-50">
          {article.category}
        </p>
      )}

      <h1 className="text-fg mb-4 font-mono text-3xl sm:text-4xl lg:text-5xl">
        {article.title}
      </h1>

      {article.author.length > 0 && (
        <div className="mb-4 flex items-center justify-start gap-3">
          {article.author.map((name: string) => {
            const avatarUrl = AUTHOR_AVATARS[name];
            return (
              <div key={name} className="flex items-center justify-start gap-2">
                {avatarUrl && (
                  <img
                    src={avatarUrl}
                    alt={name}
                    className="h-8 w-8 rounded-full object-cover"
                  />
                )}
                <p className="text-fg text-base">{name}</p>
              </div>
            );
          })}
        </div>
      )}

      <time dateTime={article.date} className="text-fg font-mono text-sm">
        {new Date(article.date).toLocaleDateString("en-US", {
          year: "numeric",
          month: "long",
          day: "numeric",
        })}
      </time>
    </header>
  );
}

function ArticleContent({ article }: { article: any }) {
  return (
    <article className="prose surface border-color-brand prose-neutral prose-p:text-base prose-headings:font-mono prose-headings:font-semibold prose-h1:text-3xl prose-h1:mt-16 prose-h1:mb-12 prose-h2:text-2xl prose-h2:mt-10 prose-h2:mb-8 prose-h3:text-xl prose-h3:mt-8 prose-h3:mb-6 prose-h4:text-lg prose-h4:mt-6 prose-h4:mb-3 prose-a:text-fg prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-color-brand prose-code:rounded prose-code:px-1.5 prose-code:py-0.5 prose-code:text-sm prose-code:font-mono prose-code:text-stone-700 prose-pre:bg-stone-50 prose-pre:border prose-pre:border-neutral-200 prose-pre:rounded-xs prose-pre:prose-code:bg-transparent prose-pre:prose-code:border-0 prose-pre:prose-code:p-0 prose-img:rounded-xs prose-img:border prose-img:border-neutral-200 prose-img:my-8 w-full max-w-none rounded-xl border py-16">
      <div className="mx-auto max-w-200">
        <MDXContent code={article.mdx} components={defaultMDXComponents} />
      </div>
    </article>
  );
}

function RelatedArticlesSection({
  relatedArticles,
}: {
  relatedArticles: any[];
}) {
  if (relatedArticles.length === 0) {
    return null;
  }

  return (
    <div className="mt-16 border-t border-neutral-100 pt-8">
      <div className="mb-6 flex items-center justify-between">
        <h3 className="font-mono text-xl text-stone-600">More articles</h3>
        <Link
          to="/blog/"
          className="text-sm text-neutral-600 transition-colors hover:text-stone-600"
        >
          See all
        </Link>
      </div>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        {relatedArticles.map((related) => (
          <RelatedArticleCard key={related.slug} article={related} />
        ))}
      </div>
    </div>
  );
}

function TableOfContents({
  toc,
}: {
  toc: Array<{ id: string; text: string; level: number }>;
}) {
  const blogTocCtx = useBlogToc();
  const [activeId, setActiveIdLocal] = useState<string | null>(
    toc.length > 0 ? toc[0].id : null,
  );
  const observerRef = useRef<IntersectionObserver | null>(null);
  const headingElementsRef = useRef<Record<string, IntersectionObserverEntry>>(
    {},
  );
  const isUserScrollingToc = useRef(false);
  const userScrollTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const wheelAccumulator = useRef(0);

  const setActiveId = useCallback(
    (id: string | null) => {
      setActiveIdLocal(id);
      blogTocCtx?.setActiveId(id);
    },
    [blogTocCtx],
  );

  useEffect(() => {
    blogTocCtx?.setToc(toc);
    return () => {
      blogTocCtx?.setToc([]);
      blogTocCtx?.setActiveId(null);
    };
  }, [toc]);

  const scrollToHeading = useCallback((id: string) => {
    isUserScrollingToc.current = true;
    if (userScrollTimeout.current) {
      clearTimeout(userScrollTimeout.current);
    }
    userScrollTimeout.current = setTimeout(() => {
      isUserScrollingToc.current = false;
    }, 1000);

    setActiveId(id);
    document.getElementById(id)?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
  }, []);

  const getActiveHeading = useCallback(() => {
    const visibleHeadings: IntersectionObserverEntry[] = [];
    for (const entry of Object.values(headingElementsRef.current)) {
      if (entry.isIntersecting) {
        visibleHeadings.push(entry);
      }
    }

    if (visibleHeadings.length > 0) {
      const sorted = visibleHeadings.sort(
        (a, b) =>
          (a.target as HTMLElement).getBoundingClientRect().top -
          (b.target as HTMLElement).getBoundingClientRect().top,
      );
      return sorted[0].target.id;
    }
    return null;
  }, []);

  useEffect(() => {
    if (toc.length === 0) return;

    observerRef.current = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          headingElementsRef.current[entry.target.id] = entry;
        }
        if (!isUserScrollingToc.current) {
          const active = getActiveHeading();
          if (active) {
            setActiveId(active);
          }
        }
      },
      { rootMargin: "-80px 0px -60% 0px", threshold: 0 },
    );

    const headingIds = toc.map((item) => item.id);
    for (const id of headingIds) {
      const el = document.getElementById(id);
      if (el) {
        observerRef.current.observe(el);
      }
    }

    return () => {
      observerRef.current?.disconnect();
    };
  }, [toc, getActiveHeading]);

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();

      const THRESHOLD = 50;
      wheelAccumulator.current += e.deltaY;

      if (Math.abs(wheelAccumulator.current) < THRESHOLD) return;

      const direction = wheelAccumulator.current > 0 ? 1 : -1;
      wheelAccumulator.current = 0;

      const currentIndex = toc.findIndex((item) => item.id === activeId);
      const nextIndex = Math.max(
        0,
        Math.min(toc.length - 1, currentIndex + direction),
      );

      if (nextIndex !== currentIndex) {
        scrollToHeading(toc[nextIndex].id);
      }
    },
    [toc, activeId, scrollToHeading],
  );

  if (toc.length === 0) {
    return null;
  }

  const activeIndex = toc.findIndex((item) => item.id === activeId);
  const ITEM_HEIGHT = 40;

  return (
    <aside
      className={cn([
        "fixed top-0 right-0 z-10 hidden h-screen xl:flex",
        "w-64 items-center",
      ])}
    >
      <nav
        className="relative w-full cursor-ns-resize overflow-hidden"
        style={{ height: ITEM_HEIGHT * 5 }}
        onWheel={handleWheel}
      >
        <motion.div
          className="flex flex-col"
          animate={{ y: -activeIndex * ITEM_HEIGHT + ITEM_HEIGHT * 2 }}
          transition={{ type: "spring", stiffness: 300, damping: 30 }}
        >
          {toc.map((item, index) => {
            const distance = Math.abs(index - activeIndex);
            const isActive = index === activeIndex;

            return (
              <a
                key={item.id}
                href={`#${item.id}`}
                onClick={(e) => {
                  e.preventDefault();
                  scrollToHeading(item.id);
                }}
                className={cn([
                  "flex shrink-0 items-center pr-4 pl-6 transition-colors duration-200",
                  isActive
                    ? "font-medium text-stone-800"
                    : "text-neutral-400 hover:text-neutral-600",
                  item.level === 3 && "pl-9",
                  item.level === 4 && "pl-12",
                ])}
                style={{
                  height: ITEM_HEIGHT,
                  opacity: isActive
                    ? 1
                    : distance === 1
                      ? 0.45
                      : distance === 2
                        ? 0.2
                        : 0.08,
                  fontSize: isActive ? 14 : 13,
                }}
              >
                <span className="line-clamp-1">{item.text}</span>
              </a>
            );
          })}
        </motion.div>
      </nav>
    </aside>
  );
}

function RelatedArticleCard({ article }: { article: any }) {
  const title = article.title ?? "";
  const ogImage =
    article.coverImage ||
    `https://char.com/og?type=blog&title=${encodeURIComponent(title)}${article.author ? `&author=${encodeURIComponent(article.author)}` : ""}${article.date ? `&date=${encodeURIComponent(new Date(article.date).toLocaleDateString("en-US", { year: "numeric", month: "long", day: "numeric" }))}` : ""}&v=1`;

  return (
    <Link
      to="/blog/$slug/"
      params={{ slug: article.slug }}
      className="group block overflow-hidden rounded-xs border border-neutral-200 bg-white transition-all hover:border-neutral-200 hover:shadow-xs"
    >
      <div className="aspect-40/21 overflow-hidden">
        <img
          src={ogImage}
          alt={title}
          className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
        />
      </div>
      <div className="p-4">
        <h4 className="mb-2 line-clamp-2 font-mono text-sm text-stone-600 transition-colors group-hover:text-stone-800">
          {title}
        </h4>
        <p className="mb-2 line-clamp-2 text-xs text-neutral-500">
          {article.summary}
        </p>
        <time dateTime={article.date} className="text-xs text-neutral-400">
          {new Date(article.date).toLocaleDateString("en-US", {
            month: "short",
            day: "numeric",
          })}
        </time>
      </div>
    </Link>
  );
}
