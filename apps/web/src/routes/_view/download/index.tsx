import { Icon } from "@iconify-icon/react";
import { DownloadSimple } from "@phosphor-icons/react";
import { createFileRoute, Link } from "@tanstack/react-router";

import { SiteFooter } from "@/components/site-footer";
import { useAnalytics } from "@/hooks/use-posthog";
import { comingSoonPlatforms, desktopDownloadSections } from "@/lib/download";
import { getCanonicalUrl } from "@/lib/seo";

const platformIcons = {
  macOS: "simple-icons:apple",
  Windows: "simple-icons:windows",
  Linux: "simple-icons:linux",
} as const;

export const Route = createFileRoute("/_view/download/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: getCanonicalUrl("/download") }],
    meta: [
      { title: "Download Anarlog" },
      {
        name: "description",
        content:
          "Download Anarlog for macOS, Windows, or Linux. Every desktop build uses the same release version.",
      },
      { property: "og:title", content: "Download Anarlog" },
      { property: "og:url", content: getCanonicalUrl("/download") },
    ],
  }),
});

function Component() {
  const { track } = useAnalytics();

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
        </section>

        <div className="grid gap-14 pb-12">
          {desktopDownloadSections.map((section) => {
            const headingId = `${section.name.toLowerCase()}-downloads`;

            return (
              <section key={section.name} aria-labelledby={headingId}>
                <h2
                  id={headingId}
                  className="font-hand mb-5 flex items-center gap-2.5 text-3xl leading-none font-semibold tracking-normal"
                >
                  <Icon
                    icon={platformIcons[section.name]}
                    className="text-2xl"
                    aria-hidden="true"
                  />
                  {section.name}
                  {section.status && (
                    <span className="border-color-subtle text-color-muted rounded-full border px-2.5 py-1 font-sans text-xs leading-none font-medium tracking-wide uppercase">
                      {section.status}
                    </span>
                  )}
                </h2>

                <ul className="border-color-subtle divide-y divide-[var(--color-border-subtle)] border-y">
                  {section.downloads.map((download) => (
                    <li key={download.name}>
                      <div className="flex items-center justify-between gap-6 px-1 py-5">
                        <span className="font-medium">{download.name}</span>
                        <a
                          href={download.url}
                          aria-label={`Download ${download.name} for ${section.name}`}
                          onClick={() =>
                            track("download_clicked", {
                              platform: section.platform,
                              spec: download.name,
                              source: "download_page",
                            })
                          }
                          className="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-[#181613] px-4 py-3 text-[13px] font-medium text-white sm:px-5 sm:text-sm"
                        >
                          Download
                          <DownloadSimple size={16} aria-hidden="true" />
                        </a>
                      </div>
                    </li>
                  ))}
                </ul>
              </section>
            );
          })}

          <section aria-labelledby="coming-soon-platforms">
            <h2
              id="coming-soon-platforms"
              className="font-hand mb-5 text-3xl leading-none font-semibold tracking-normal"
            >
              Coming soon
            </h2>

            <ul className="flex flex-wrap gap-2">
              {comingSoonPlatforms.map((platform) => (
                <li
                  key={platform}
                  className="border-color-subtle text-color-muted rounded-full border px-4 py-2 text-sm font-medium"
                >
                  {platform}
                </li>
              ))}
            </ul>
          </section>
        </div>
      </div>

      <SiteFooter />
    </main>
  );
}
