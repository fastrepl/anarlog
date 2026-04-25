import { MDXContent } from "@content-collections/mdx/react";
import { createFileRoute, notFound } from "@tanstack/react-router";
import { allArticles } from "content-collections";

import {
  CHAR_SITE_URL,
  DEFAULT_OG_IMAGE_URL,
  getBreadcrumbListJsonLd,
  getOrganizationJsonLd,
  getStructuredDataGraph,
} from "@/lib/seo";

export const Route = createFileRoute("/_view/blog/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const article = allArticles.find((article) => article.slug === params.slug);
    if (!article) {
      throw notFound();
    }

    return { article };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.article) {
      return { meta: [] };
    }

    const { article } = loaderData;
    const title = article.title ?? "";
    const description = article.meta_description ?? title;
    const url = `${CHAR_SITE_URL}/blog/${article.slug}`;
    const image = article.coverImage || DEFAULT_OG_IMAGE_URL;

    return {
      links: [{ rel: "canonical", href: url }],
      scripts: [
        {
          type: "application/ld+json",
          children: JSON.stringify(
            getStructuredDataGraph([
              {
                "@type": "BlogPosting",
                headline: title,
                description,
                image: [
                  image.startsWith("http") ? image : `${CHAR_SITE_URL}${image}`,
                ],
                datePublished: article.date,
                mainEntityOfPage: url,
                url,
                articleSection: article.category,
                author: article.author.map((name: string) => ({
                  "@type": "Person",
                  name,
                })),
                publisher: getOrganizationJsonLd(),
              },
              getBreadcrumbListJsonLd([
                { name: "Home", item: CHAR_SITE_URL },
                { name: "Articles", item: `${CHAR_SITE_URL}/blog` },
                { name: title, item: url },
              ]),
            ]),
          ),
        },
      ],
      meta: [
        { title: `${title} - Char` },
        { name: "description", content: description },
        { property: "og:title", content: title },
        { property: "og:description", content: description },
        { property: "og:type", content: "article" },
        { property: "og:url", content: url },
        { property: "og:image", content: image },
        { name: "twitter:card", content: "summary_large_image" },
        { name: "twitter:title", content: title },
        { name: "twitter:description", content: description },
        { name: "twitter:image", content: image },
        { property: "article:published_time", content: article.date },
      ],
    };
  },
});

function Component() {
  const { article } = Route.useLoaderData();

  return (
    <main className="mx-auto min-h-screen w-full max-w-4xl px-4 py-5 sm:px-6 lg:px-8">
      <header className="border-color-subtle border-b py-10 sm:py-14">
        <a
          href="/blog"
          className="text-color-muted mb-8 inline-block font-mono text-sm"
        >
          Articles
        </a>
        {article.category && (
          <p className="text-color-muted mb-4 font-mono text-sm">
            {article.category}
          </p>
        )}
        <h1 className="text-color text-4xl sm:text-5xl">{article.title}</h1>
        <div className="text-color-muted mt-5 flex flex-wrap gap-3 text-sm">
          {article.author.length > 0 && (
            <span>{article.author.join(", ")}</span>
          )}
          <time dateTime={article.date}>{formatDate(article.date)}</time>
        </div>
      </header>
      <article className="prose prose-neutral prose-headings:font-mono prose-a:text-color prose-a:underline prose-img:rounded-lg prose-img:border prose-img:border-color-subtle max-w-none py-10">
        <MDXContent code={article.mdx} components={mdxComponents} />
      </article>
    </main>
  );
}

const mdxComponents = {
  a: (props: React.ComponentProps<"a">) => (
    <a {...props} target={isExternal(props.href) ? "_blank" : props.target} />
  ),
  Image: (props: React.ComponentProps<"img">) => (
    <img {...props} className="border-color-subtle my-8 rounded-lg border" />
  ),
  CtaCard: () => null,
  Callout: ({ children }: { children: React.ReactNode }) => (
    <aside className="border-color-subtle my-6 rounded-lg border bg-white p-4">
      {children}
    </aside>
  ),
  Clip: ({ src }: { src: string }) => (
    <a href={src} target="_blank" rel="noreferrer">
      Watch clip
    </a>
  ),
};

function isExternal(href: string | undefined) {
  return Boolean(href && /^https?:\/\//.test(href));
}

function formatDate(value: string) {
  return new Date(value).toLocaleDateString("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}
