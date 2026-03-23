import { createFileRoute, Link } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/product/markdown")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Markdown Files - Char" },
      {
        name: "description",
        content:
          "Your notes are stored as plain markdown files. Own your data, export anytime, and use with any markdown-compatible tool.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  return (
    <div
      className="overflow-hidden bg-linear-to-b from-white via-stone-50/20 to-white sm:h-[calc(100vh-65px)]"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="relative mx-auto flex h-full max-w-6xl flex-col border-x border-neutral-100 bg-white">
        <div className="relative z-10 flex flex-1 items-center justify-center bg-[linear-gradient(to_bottom,rgba(245,245,244,0.2),white_50%,rgba(245,245,244,0.3))] px-6 py-12">
          <div className="pointer-events-auto mx-auto max-w-4xl text-center">
            <h1 className="mx-auto mb-6 max-w-2xl font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
              Markdown Files
            </h1>
            <p className="mx-auto mb-8 max-w-2xl text-lg text-neutral-600 sm:text-xl">
              Your notes are stored as plain markdown files. Own your data,
              export anytime, and use with any markdown-compatible tool.
            </p>

            <div className="mt-8">
              <Link
                to="/download/"
                className={cn([
                  "inline-block rounded-full px-8 py-3 text-base font-medium",
                  "bg-linear-to-t from-stone-600 to-stone-500 text-white",
                  "transition-transform hover:scale-105 active:scale-95",
                ])}
              >
                Download for free
              </Link>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
