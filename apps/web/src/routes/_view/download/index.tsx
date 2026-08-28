import { Icon } from "@iconify-icon/react";
import { ArrowSquareOut, DownloadSimple } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { createFileRoute, Link } from "@tanstack/react-router";

import { SiteFooter } from "@/components/site-footer";
import { useAnalytics } from "@/hooks/use-posthog";
import { comingSoonPlatforms, desktopDownloadSections } from "@/lib/download";
import { getCanonicalUrl } from "@/lib/seo";
const styles = stylex.create({
  style1: {
    minHeight: "100vh",
  },
  style2: {
    marginInline: "auto",
    width: "100%",
    maxWidth: "700px",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    paddingBlock: {
      default: "2rem",
      "@media (width >= 48rem)": "3rem",
    },
  },
  style3: {
    height: "2.25rem",
    width: "auto",
  },
  style4: {
    paddingTop: {
      default: "6rem",
      "@media (width >= 48rem)": "8rem",
    },
    paddingBottom: "4rem",
  },
  style5: {
    fontSize: {
      default: "3.75rem",
      "@media (width >= 48rem)": "6rem",
    },
    lineHeight: {
      default: ".98",
      "@media (width >= 48rem)": "1",
    },
    "--tw-leading": ".98",
    "--tw-font-weight": "600",
    fontWeight: "600",
    textWrap: "balance",
  },
  style6: {
    display: "grid",
    gap: "3.5rem",
    paddingBottom: "3rem",
  },
  style7: {
    marginBottom: "1.25rem",
    display: "flex",
    alignItems: "center",
    gap: ".625rem",
    fontSize: "1.875rem",
    lineHeight: "1",
    "--tw-leading": "1",
    "--tw-font-weight": "600",
    fontWeight: "600",
  },
  style8: {
    fontSize: "1.5rem",
    lineHeight: "2rem",
  },
  style9: {
    borderRadius: "3.40282e38px",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: ".625rem",
    paddingBlock: ".25rem",
    fontFamily:
      "ui-sans-serif, system-ui, sans-serif, Apple Color Emoji, Segoe UI Emoji, Segoe UI Symbol, Noto Color Emoji",
    fontSize: ".75rem",
    lineHeight: "1",
    "--tw-leading": "1",
    "--tw-font-weight": "500",
    fontWeight: "500",
    textTransform: "uppercase",
  },
  style10: {
    borderBlockStyle: "solid",
    borderBlockWidth: "1px",
  },
  style11: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "1.5rem",
    paddingInline: ".25rem",
    paddingBlock: "1.25rem",
  },
  style12: {
    "--tw-font-weight": "500",
    fontWeight: "500",
  },
  style13: {
    display: "inline-flex",
    flexShrink: "0",
    alignItems: "center",
    gap: ".375rem",
    borderRadius: "3.40282e38px",
    backgroundColor: "#181613",
    paddingInline: {
      default: "1rem",
      "@media (width >= 40rem)": "1.25rem",
    },
    paddingBlock: ".75rem",
    fontSize: {
      default: "13px",
      "@media (width >= 40rem)": ".875rem",
    },
    "--tw-font-weight": "500",
    fontWeight: "500",
    color: "#fff",
    lineHeight: {
      default: null,
      "@media (width >= 40rem)": "1.25rem",
    },
  },
  style14: {
    marginTop: "1rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
  },
  style15: {
    textDecorationLine: "underline",
    textUnderlineOffset: "4px",
  },
  style16: {
    marginBottom: "1.25rem",
    fontSize: "1.875rem",
    lineHeight: "1",
    "--tw-leading": "1",
    "--tw-font-weight": "600",
    fontWeight: "600",
  },
  style17: {
    display: "flex",
    flexWrap: "wrap",
    gap: ".5rem",
  },
  style18: {
    borderRadius: "3.40282e38px",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    "--tw-font-weight": "500",
    fontWeight: "500",
  },
});
const platformIcons = {
  macOS: "simple-icons:apple",
  Windows: "simple-icons:windows",
  Linux: "simple-icons:linux",
} as const;
export const Route = createFileRoute("/_view/download/")({
  component: Component,
  head: () => ({
    links: [
      {
        rel: "canonical",
        href: getCanonicalUrl("/download"),
      },
    ],
    meta: [
      {
        title: "Download Anarlog",
      },
      {
        name: "description",
        content:
          "Download Anarlog for macOS, Windows, or Linux. Every desktop build uses the same release version.",
      },
      {
        property: "og:title",
        content: "Download Anarlog",
      },
      {
        property: "og:url",
        content: getCanonicalUrl("/download"),
      },
    ],
  }),
});
function Component() {
  const { track } = useAnalytics();
  return (
    <main {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <header>
          <Link to="/" aria-label="Anarlog home">
            <img
              src="/logo.svg"
              alt="Anarlog"
              {...stylex.props(styles.style3)}
            />
          </Link>
        </header>

        <section {...stylex.props(styles.style4)}>
          <h1 {...stylex.props(styles.style5)}>Download Anarlog</h1>
        </section>

        <div {...stylex.props(styles.style6)}>
          {desktopDownloadSections.map((section) => {
            const headingId = `${section.name.toLowerCase()}-downloads`;
            return (
              <section key={section.name} aria-labelledby={headingId}>
                <h2 id={headingId} {...stylex.props(styles.style7)}>
                  <Icon
                    icon={platformIcons[section.name]}
                    {...stylex.props(styles.style8)}
                    aria-hidden="true"
                  />
                  {section.name}
                  {section.status && (
                    <span {...stylex.props(styles.style9)}>
                      {section.status}
                    </span>
                  )}
                </h2>

                <ul {...stylex.props(styles.style10)}>
                  {section.downloads.map((download) => (
                    <li key={download.name}>
                      <div {...stylex.props(styles.style11)}>
                        <span {...stylex.props(styles.style12)}>
                          {download.name}
                        </span>
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
                            })
                          }
                          {...stylex.props(styles.style13)}
                        >
                          {"actionLabel" in download ? (
                            <>
                              {download.actionLabel}
                              <ArrowSquareOut size={16} aria-hidden="true" />
                            </>
                          ) : (
                            <>
                              Download
                              <DownloadSimple size={16} aria-hidden="true" />
                            </>
                          )}
                        </a>
                      </div>
                    </li>
                  ))}
                </ul>
                {section.platform === "linux" && (
                  <p {...stylex.props(styles.style14)}>
                    If the window is black on Wayland with NVIDIA, see{" "}
                    <a
                      href="https://docs.anarlog.so/desktop-installation#appimage"
                      {...stylex.props(styles.style15)}
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
            <h2 id="coming-soon-platforms" {...stylex.props(styles.style16)}>
              Coming soon
            </h2>

            <ul {...stylex.props(styles.style17)}>
              {comingSoonPlatforms.map((platform) => (
                <li key={platform} {...stylex.props(styles.style18)}>
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
