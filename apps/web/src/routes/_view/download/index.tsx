import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Download } from "lucide-react";

import { SiteFooter } from "@/components/site-footer";
import { desktopDownloadSections } from "@/lib/download";
import { ANARLOG_SITE_URL } from "@/lib/seo";

const platformIcons = {
  macOS: "simple-icons:apple",
  Windows: "simple-icons:windows",
  Linux: "simple-icons:linux",
} as const;

export const Route = createFileRoute("/_view/download/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: `${ANARLOG_SITE_URL}/download` }],
    meta: [
      { title: "Download Anarlog" },
      {
        name: "description",
        content:
          "Download Anarlog for macOS, Windows, or Linux. Every desktop build uses the same release version.",
      },
      { property: "og:title", content: "Download Anarlog" },
      { property: "og:url", content: `${ANARLOG_SITE_URL}/download` },
    ],
  }),
});

function Component() {
  return (
    <main className="surface text-color min-h-screen">
      <div className="mx-auto w-full max-w-[700px] px-5 py-8 md:px-8 md:py-12">
        <header>
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <section className="pt-24 pb-16 md:pt-32">
          <h1 className="font-hand text-color text-6xl leading-[0.98] font-semibold tracking-normal text-balance md:text-8xl">
            Download Anarlog
          </h1>
          <p className="text-color-secondary mt-6 max-w-2xl text-xl leading-9">
            macOS, Windows Preview, and Linux Beta ship together with the same
            desktop version.
          </p>
        </section>

        <div className="grid gap-14 pb-12">
          {desktopDownloadSections.map((section) => {
            const headingId = `${section.name.toLowerCase()}-downloads`;

            return (
              <section key={section.name} aria-labelledby={headingId}>
                <div className="mb-5">
                  <h2
                    id={headingId}
                    className="font-hand flex items-center gap-2.5 text-3xl leading-none font-semibold tracking-normal"
                  >
                    <Icon
                      icon={platformIcons[section.name]}
                      className="text-2xl"
                      aria-hidden="true"
                    />
                    {section.name}
                  </h2>
                  <p className="text-color-secondary mt-2 leading-7">
                    {section.description}
                  </p>
                </div>

                <ul className="border-color-subtle divide-y divide-[var(--color-border-subtle)] border-y">
                  {section.downloads.map((download) => (
                    <li key={download.name}>
                      <a
                        href={download.url}
                        className="hover:bg-surface-subtle flex items-center justify-between gap-6 px-1 py-5 transition-colors"
                      >
                        <span className="min-w-0">
                          <span className="block font-medium">
                            {download.name}
                          </span>
                          <span className="text-color-secondary mt-1 block text-sm">
                            {download.detail}
                          </span>
                        </span>
                        <span className="inline-flex shrink-0 items-center gap-2 font-mono text-sm font-medium">
                          Download
                          <Download
                            size={16}
                            strokeWidth={2}
                            aria-hidden="true"
                          />
                        </span>
                      </a>
                    </li>
                  ))}
                </ul>
              </section>
            );
          })}
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}
