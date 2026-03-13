import { createFileRoute } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/product/memory")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Memory - Char" },
      {
        name: "description",
        content:
          "Your memory layer that connects all your meetings and conversations. Coming soon.",
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
              Your memory layer
            </h1>
            <p className="mx-auto mb-8 max-w-2xl text-lg text-neutral-600 sm:text-xl">
              Char connects all your meetings and conversations. The more you
              use it, the more it knows about you, your team, and your work.
            </p>

            <div className="mt-8">
              <button
                disabled
                className={cn([
                  "inline-block cursor-not-allowed px-8 py-3 text-base font-medium",
                  "rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs",
                ])}
              >
                Coming Soon
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
