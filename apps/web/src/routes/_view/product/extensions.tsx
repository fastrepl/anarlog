import { createFileRoute } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/product/extensions")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Extensions - Char" },
      {
        name: "description",
        content:
          "Connect Char with your favorite tools and build custom integrations with our API. Extensions coming soon.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  const extensionAreas = [
    {
      label: "Custom Views",
      description: "Graph views, kanban boards, and more",
    },
    { label: "Themes", description: "Personalize your workspace" },
    {
      label: "Editor Plugins",
      description: "Enhanced editing capabilities",
    },
    { label: "Integrations", description: "Connect external tools" },
    { label: "Data Sources", description: "Import from anywhere" },
    { label: "Export Formats", description: "Share in any format" },
  ];

  return (
    <div
      className="h-[calc(100vh-65px)] bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto h-full max-w-6xl border-x border-neutral-100 bg-white">
        <div className="flex h-full items-center justify-center bg-[linear-gradient(to_bottom,rgba(245,245,244,0.2),white_50%,rgba(245,245,244,0.3))] px-6">
          <div className="mx-auto max-w-4xl py-12 text-center">
            <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-600 sm:text-5xl">
              Build Beyond the Defaults
            </h1>
            <p className="mb-10 text-lg text-neutral-600 sm:text-xl">
              Extend Char with custom themes, plugins, and views. Build together
              with the community.
            </p>

            <div className="mb-10">
              <div className="mx-auto flex max-w-2xl flex-wrap justify-center gap-2">
                {extensionAreas.map((area) => (
                  <div
                    key={area.label}
                    className="rounded-full border border-stone-200 bg-stone-50 px-3 py-1.5 text-xs font-medium text-stone-600"
                  >
                    {area.label}
                  </div>
                ))}
              </div>
            </div>

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
