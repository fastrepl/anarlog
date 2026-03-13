import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

import { MockWindow } from "@/components/mock-window";

const TITLE = "Press Kit - Char";
const DESCRIPTION =
  "Download Char press materials, logos, screenshots, and brand assets.";

export const Route = createFileRoute("/_view/press-kit/")({
  component: Component,
  head: () => ({
    meta: [
      { title: TITLE },
      {
        name: "description",
        content: DESCRIPTION,
      },
      { name: "robots", content: "noindex, nofollow" },
      {
        property: "og:title",
        content: TITLE,
      },
      {
        property: "og:description",
        content: DESCRIPTION,
      },
      {
        name: "twitter:title",
        content: TITLE,
      },
      {
        name: "twitter:description",
        content: DESCRIPTION,
      },
    ],
  }),
});

function Component() {
  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <div className="px-6 py-16 lg:py-24">
          <div className="mx-auto max-w-3xl text-center">
            <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
              Press Kit
            </h1>
            <p className="text-lg text-neutral-600 sm:text-xl">
              Download press materials, logos, screenshots, and learn more about
              Char. For press inquiries, contact us at{" "}
              <a
                href="mailto:founders@char.com"
                className="text-stone-600 underline hover:text-stone-700"
              >
                founders@char.com
              </a>
            </p>
          </div>
        </div>

        <section className="px-6 pb-16 lg:pb-24">
          <div className="mx-auto max-w-4xl">
            <MockWindow className="w-full max-w-none rounded-lg">
              <div className="p-8">
                <div className="mb-8">
                  <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
                    Press Materials
                  </div>
                  <div className="grid grid-cols-2 gap-6 sm:grid-cols-4">
                    <FinderFolder
                      to="/press-kit/app/"
                      folderImage="/api/images/icons/macos-folder-blue.png"
                      label="App"
                    />
                    <FinderFolder
                      to="/brand/"
                      folderImage="/api/images/icons/macos-folder-red.png"
                      label="Brand"
                    />
                    <FinderFolder
                      to="/about/"
                      folderImage="/api/images/icons/macos-folder-purple.png"
                      label="Team"
                    />
                    <div className="invisible">
                      <div className="mb-3 h-16 w-16"></div>
                      <div className="font-medium text-stone-600">
                        Placeholder
                      </div>
                    </div>
                  </div>
                </div>

                <div className="border-t border-neutral-100 pt-8">
                  <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
                    Quick Actions
                  </div>
                  <div className="grid grid-cols-2 gap-6 sm:grid-cols-4">
                    <FinderAction
                      href="/download/apple-silicon"
                      label="Download"
                      download
                      appIcon
                    />
                    <FinderAction
                      href="mailto:founders@char.com"
                      iconImage="/api/images/icons/macos-mail.png"
                      label="Contact"
                    />
                    <FinderAction
                      href="https://github.com/fastrepl/char"
                      iconImage="/api/images/icons/github.webp"
                      label="GitHub"
                      external
                      roundedIcon
                    />
                    <div className="invisible">
                      <div className="mb-3 h-16 w-16"></div>
                      <div className="font-medium text-stone-600">
                        Placeholder
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <div className="border-t border-neutral-200 bg-stone-50 px-4 py-2">
                <span className="text-xs text-neutral-500">
                  7 items, 2 groups
                </span>
              </div>
            </MockWindow>
          </div>
        </section>
      </div>
    </div>
  );
}

function FinderFolder({
  to,
  folderImage,
  label,
}: {
  to: string;
  folderImage: string;
  label: string;
}) {
  return (
    <Link
      to={to}
      className={cn([
        "group flex flex-col items-center rounded-lg p-4 text-center",
        "transition-colors hover:bg-stone-50",
        "cursor-pointer",
      ])}
    >
      <div className="mb-3 flex h-16 w-16 items-center justify-center">
        <img
          src={folderImage}
          alt={`${label} folder`}
          className="h-16 w-16 transition-transform group-hover:scale-110"
        />
      </div>
      <div className="font-medium text-stone-600">{label}</div>
    </Link>
  );
}

function FinderAction({
  href,
  iconImage,
  label,
  download,
  external,
  appIcon,
  roundedIcon,
}: {
  href: string;
  iconImage?: string;
  label: string;
  download?: boolean;
  external?: boolean;
  appIcon?: boolean;
  roundedIcon?: boolean;
}) {
  const content = (
    <>
      <div className="mb-3">
        {appIcon ? (
          <img
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            className="mx-auto h-16 w-16 rounded-[20px] border border-neutral-100 shadow-md transition-transform group-hover:scale-110"
          />
        ) : iconImage ? (
          <img
            src={iconImage}
            alt={label}
            className={cn([
              "mx-auto h-16 w-16 transition-transform group-hover:scale-110",
              roundedIcon ? "rounded-[20px] shadow-md" : "",
            ])}
          />
        ) : null}
      </div>
      <div className="font-medium text-stone-600">{label}</div>
    </>
  );

  const className = cn([
    "group flex flex-col items-center rounded-lg p-4 text-center",
    "transition-colors hover:bg-stone-50",
    "cursor-pointer",
  ]);

  if (external) {
    return (
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer"
        className={className}
      >
        {content}
      </a>
    );
  }

  if (download) {
    return (
      <a href={href} download className={className}>
        {content}
      </a>
    );
  }

  return (
    <a href={href} className={className}>
      {content}
    </a>
  );
}
