import { createFileRoute, Link } from "@tanstack/react-router";
import { allArticles } from "content-collections";

import { SiteFooter } from "@/components/site-footer";
import { ANARLOG_SITE_URL } from "@/lib/seo";

export const Route = createFileRoute("/blog/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: `${ANARLOG_SITE_URL}/blog` }],
    meta: [
      { title: "Anarlog Blog" },
      {
        name: "description",
        content:
          "Guides for AI meeting notes, privacy research, and engineering notes from the Anarlog team.",
      },
      { property: "og:title", content: "Anarlog Blog" },
      { property: "og:url", content: `${ANARLOG_SITE_URL}/blog` },
    ],
  }),
});

function Component() {
  const sortedArticles = [...allArticles].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
  );

  return (
    <main className="min-h-screen bg-white text-[#181613]">
      <div className="mx-auto w-full max-w-[700px] px-5 py-8 md:px-8 md:py-12">
        <header className="flex items-center justify-between gap-6">
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <section className="pt-24 pb-16 md:pt-32">
          <h1 className="font-hand text-6xl leading-[0.98] font-semibold tracking-normal text-balance text-black md:text-8xl">
            Blog
          </h1>
          <p className="mt-6 max-w-2xl text-xl leading-9 text-[#363029]">
            Notes on private meetings, local files, open source, and AI you can
            run on your own terms.
          </p>
        </section>

        <ul className="grid gap-9">
          {sortedArticles.map((article) => (
            <li key={article.slug}>
              <Link
                to="/blog/$slug/"
                params={{ slug: article.slug }}
                className="group block"
              >
                <article className="grid gap-3 border-t border-[#eee8df] pt-6">
                  <h2 className="font-hand text-3xl leading-[1.05] font-semibold tracking-normal text-balance text-[#756b5d] group-hover:text-[#4f4940]">
                    {article.title}
                  </h2>
                  {article.meta_description && (
                    <p className="line-clamp-2 leading-7 text-[#4f4940]">
                      {article.meta_description}
                    </p>
                  )}
                  <div className="flex items-center gap-2 text-xs text-[#756b5d]">
                    <span>
                      {Array.isArray(article.author)
                        ? article.author.join(", ")
                        : article.author}
                    </span>
                    <span>·</span>
                    <time dateTime={article.date}>
                      {new Date(article.date).toLocaleDateString("en-US", {
                        month: "short",
                        day: "numeric",
                        year: "numeric",
                      })}
                    </time>
                  </div>
                </article>
              </Link>
            </li>
          ))}
        </ul>
      </div>

      <SiteFooter />
    </main>
  );
}
