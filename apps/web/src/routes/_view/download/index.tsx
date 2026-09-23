import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import {
  ArrowSquareOut,
  Clock,
  DownloadSimple,
} from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { SiteFooter } from "@/components/site-footer";
import { useExperiment } from "@/hooks/use-experiment";
import { useAnalytics } from "@/hooks/use-posthog";
import {
  comingSoonPlatforms,
  desktopDownloadSections,
  mobileDownloadSections,
  platformIcons,
} from "@/lib/download";
import { experimentProperties } from "@/lib/experiments";
import { getCanonicalUrl } from "@/lib/seo";

const downloadSections = [
  ...desktopDownloadSections,
  ...mobileDownloadSections,
];

const downloadButtonClassName =
  "items-center gap-1.5 rounded-full bg-[#181613] px-4 py-3 text-[13px] font-medium text-white sm:text-sm";

const gridCellClassName =
  "border-color-subtle flex flex-col gap-7 border-b py-10 md:border-l md:px-9 md:[&:nth-child(3n+1)]:border-l-0 md:[&:nth-child(3n+1)]:pl-0 md:[&:nth-child(3n)]:pr-0";

export const Route = createFileRoute("/_view/download/")({
  component: Component,
  head: () => ({
    links: [{ rel: "canonical", href: getCanonicalUrl("/download") }],
    meta: [
      { title: "Download Anarlog" },
      {
        name: "description",
        content:
          "Download Anarlog for macOS, Windows, or Linux. iOS and Android apps are coming in October.",
      },
      { property: "og:title", content: "Download Anarlog" },
      { property: "og:url", content: getCanonicalUrl("/download") },
    ],
  }),
});

const nextDevices = ["Watch", "Dongle", "Pin"] as const;

function NextDeviceButton() {
  const [index, setIndex] = useState(0);

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      return;
    }
    const id = window.setInterval(
      () => setIndex((current) => (current + 1) % nextDevices.length),
      1800,
    );
    return () => window.clearInterval(id);
  }, []);

  return (
    <span
      aria-disabled="true"
      aria-label={`Coming next: ${nextDevices.join(", ")}`}
      className={cn([
        downloadButtonClassName,
        "mt-auto flex w-full cursor-not-allowed justify-between opacity-40",
      ])}
    >
      <span>{nextDevices[index]}</span>
      <Clock size={16} aria-hidden="true" />
    </span>
  );
}

function Component() {
  const { track } = useAnalytics();
  const { variant, ready } = useExperiment("downloadLayout");
  const experiment = ready
    ? experimentProperties("downloadLayout", variant)
    : {};
  const threeColumn = variant === "three-column";

  return (
    <main className="surface text-color min-h-screen">
      <div
        className={cn([
          "mx-auto w-full px-5 py-8 md:px-8 md:py-12",
          threeColumn ? "max-w-[1040px]" : "max-w-[700px]",
        ])}
      >
        <header>
          <Link to="/" aria-label="Anarlog home">
            <img src="/logo.svg" alt="Anarlog" className="h-9 w-auto" />
          </Link>
        </header>

        <section
          className={cn([
            threeColumn ? "pt-20 pb-14 md:pt-28" : "pt-24 pb-16 md:pt-32",
          ])}
        >
          <h1 className="font-hand text-color text-6xl leading-[0.98] font-semibold tracking-normal text-balance md:text-8xl">
            Download Anarlog
          </h1>
        </section>

        {threeColumn && (
          <section aria-labelledby="all-downloads" className="mb-12">
            <h2 id="all-downloads" className="sr-only">
              Downloads
            </h2>
            <div className="border-color-subtle grid border-t md:grid-cols-3">
              {downloadSections.map((section) => (
                <div key={section.name} className={gridCellClassName}>
                  <h3 className="font-hand flex items-center gap-2.5 text-3xl leading-none font-semibold tracking-normal">
                    <Icon
                      icon={platformIcons[section.name]}
                      className="text-2xl"
                      aria-hidden="true"
                    />
                    {section.name}
                  </h3>
                  <ul className="mt-auto flex flex-col gap-2">
                    {section.downloads
                      .filter((download) => download.showInMenu)
                      .map((download) => (
                        <li key={download.name}>
                          {!section.available ? (
                            <span
                              aria-disabled="true"
                              className={cn([
                                downloadButtonClassName,
                                "flex w-full cursor-not-allowed justify-between opacity-40",
                              ])}
                            >
                              {section.status}
                              <Clock size={16} aria-hidden="true" />
                            </span>
                          ) : (
                            <a
                              href={download.url}
                              {...("actionLabel" in download
                                ? { target: "_blank", rel: "noreferrer" }
                                : {})}
                              aria-label={
                                "actionLabel" in download
                                  ? `${download.actionLabel}: ${download.name}`
                                  : `Download ${download.name} for ${section.name}`
                              }
                              onClick={() =>
                                track("download_clicked", {
                                  platform: section.platform,
                                  spec: download.name,
                                  source: "download_page",
                                  ...experiment,
                                })
                              }
                              className={cn([
                                downloadButtonClassName,
                                "flex justify-between",
                              ])}
                            >
                              {"actionLabel" in download
                                ? download.actionLabel
                                : download.name}
                              {"actionLabel" in download ? (
                                <ArrowSquareOut size={16} aria-hidden="true" />
                              ) : (
                                <DownloadSimple size={16} aria-hidden="true" />
                              )}
                            </a>
                          )}
                        </li>
                      ))}
                  </ul>
                </div>
              ))}
              <div className={gridCellClassName}>
                <h3 className="font-hand text-3xl leading-none font-semibold tracking-normal">
                  Next?
                </h3>
                <NextDeviceButton />
              </div>
            </div>
            {downloadSections
              .filter(
                (section) =>
                  section.available &&
                  section.downloads.some((download) => !download.showInMenu),
              )
              .map((section) => (
                <p
                  key={section.name}
                  className="text-color-muted mt-6 text-sm leading-6"
                >
                  More {section.name} installers:{" "}
                  {section.downloads
                    .filter((download) => !download.showInMenu)
                    .map((download, index) => (
                      <span key={download.name}>
                        {index > 0 && " · "}
                        <a
                          href={download.url}
                          {...("actionLabel" in download
                            ? { target: "_blank", rel: "noreferrer" }
                            : {})}
                          onClick={() =>
                            track("download_clicked", {
                              platform: section.platform,
                              spec: download.name,
                              source: "download_page",
                              ...experiment,
                            })
                          }
                          className="text-color underline underline-offset-2"
                        >
                          {download.name}
                        </a>
                      </span>
                    ))}
                </p>
              ))}
          </section>
        )}

        {!threeColumn && (
          <div className="grid gap-14 pb-12">
            {downloadSections.map((section) => {
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
                  </h2>

                  {(section.platform === "ios" ||
                    section.platform === "android") && (
                    <p className="text-color-muted mb-4 text-sm leading-6">
                      {section.description}
                    </p>
                  )}

                  <ul className="border-color-subtle divide-y divide-[var(--color-border-subtle)] border-y">
                    {section.downloads.map((download) => (
                      <li key={download.name}>
                        <div className="flex items-center justify-between gap-6 px-1 py-5">
                          <span className="font-medium">{download.name}</span>
                          {!section.available ? (
                            <span
                              aria-disabled="true"
                              className={cn([
                                downloadButtonClassName,
                                "inline-flex shrink-0 cursor-not-allowed opacity-40 sm:px-5",
                              ])}
                            >
                              {section.status}
                              <Clock size={16} aria-hidden="true" />
                            </span>
                          ) : (
                            <a
                              href={download.url}
                              {...("actionLabel" in download
                                ? {
                                    target: "_blank",
                                    rel: "noreferrer",
                                  }
                                : {})}
                              aria-label={
                                "actionLabel" in download
                                  ? `${download.actionLabel}: ${download.name}`
                                  : `Download ${download.name} for ${section.name}`
                              }
                              onClick={() =>
                                track("download_clicked", {
                                  platform: section.platform,
                                  spec: download.name,
                                  source: "download_page",
                                  ...experiment,
                                })
                              }
                              className={cn([
                                downloadButtonClassName,
                                "inline-flex shrink-0 sm:px-5",
                              ])}
                            >
                              {"actionLabel" in download ? (
                                <>
                                  {download.actionLabel}
                                  <ArrowSquareOut
                                    size={16}
                                    aria-hidden="true"
                                  />
                                </>
                              ) : (
                                <>
                                  Download
                                  <DownloadSimple
                                    size={16}
                                    aria-hidden="true"
                                  />
                                </>
                              )}
                            </a>
                          )}
                        </div>
                      </li>
                    ))}
                  </ul>
                  {section.platform === "linux" && (
                    <p className="text-color-muted mt-4 text-sm leading-6">
                      If the window is black on Wayland with NVIDIA, see{" "}
                      <a
                        href="https://docs.anarlog.so/desktop-installation#appimage"
                        className="text-color underline underline-offset-4"
                      >
                        Linux install notes
                      </a>
                      .
                    </p>
                  )}
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
        )}
      </div>

      <SiteFooter />
    </main>
  );
}
