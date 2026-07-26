import { createFileRoute } from "@tanstack/react-router";
import { type Article, allArticles } from "content-collections";

import { renderBlogOgImage } from "@/lib/og-image";

export const Route = createFileRoute("/api/og/blog/$slug")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const article = allArticles.find(
          (a: Article) => a.slug === params.slug,
        );

        if (!article) {
          return new Response("Not found", { status: 404 });
        }

        return renderBlogOgImage({
          title: article.meta_title || article.title,
          description: article.meta_description,
          date: article.date,
          author: Array.isArray(article.author)
            ? article.author.join(", ")
            : article.author,
        });
      },
    },
  },
});
