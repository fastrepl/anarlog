import { createFileRoute } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";

export const Route = createFileRoute("/_view/product/integrations")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Integrations & Workflows - Char" },
      {
        name: "description",
        content:
          "Connect Char with your favorite tools and automate your meeting workflow. Integrations coming soon.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

const integrations = [
  { name: "Slack", image: "slack.jpg" },
  { name: "Linear", image: "linear.jpg" },
  { name: "Notion", image: "notion.jpg" },
  { name: "Salesforce", image: "salesforce.jpg" },
  { name: "Affinity", image: "affinity.jpg" },
  { name: "Attio", image: "attio.jpg" },
  { name: "Google Calendar", image: "gcal.jpg" },
  { name: "Gmail", image: "gmail.jpg" },
  { name: "HubSpot", image: "hubspot.jpg" },
  { name: "Jira", image: "jira.jpg" },
  { name: "Obsidian", image: "obsidian.png" },
];

function IntegrationIcon({
  integration,
}: {
  integration: { name: string; image: string };
}) {
  return (
    <div className="size-20 shrink-0 overflow-hidden rounded-xl border border-neutral-100/50 bg-white shadow-xs transition-all hover:scale-110 hover:border-neutral-400">
      <Image
        src={`/api/images/icons/${integration.image}`}
        alt={integration.name}
        width={80}
        height={80}
        className="h-full w-full object-cover"
      />
    </div>
  );
}

function IntegrationsGrid() {
  const rows = 10;
  const cols = 16;

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
      <div className="absolute inset-0 flex flex-col justify-center gap-2 px-4 opacity-50">
        {Array.from({ length: rows }).map((_, rowIndex) => (
          <div key={rowIndex} className="flex justify-center gap-2">
            {Array.from({ length: cols }).map((_, colIndex) => {
              const index = (rowIndex * cols + colIndex) % integrations.length;
              const integration = integrations[index];
              const delay = Math.random() * 3;

              return (
                <div
                  key={`${rowIndex}-${colIndex}`}
                  className="animate-fade-in-out pointer-events-auto"
                  style={{
                    animationDelay: `${delay}s`,
                    animationDuration: "3s",
                  }}
                >
                  <IntegrationIcon integration={integration} />
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}

function Component() {
  return (
    <div
      className="relative h-[calc(100vh-65px)] overflow-hidden bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="relative mx-auto h-full max-w-6xl border-x border-neutral-100 bg-white">
        <div className="relative h-full overflow-hidden bg-linear-to-b from-stone-50/30 to-stone-100/30">
          <IntegrationsGrid />
          <div className="relative z-10 flex h-full items-center justify-center px-6">
            <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_800px_400px_at_50%_50%,white_0%,rgba(255,255,255,0.8)_40%,transparent_70%)]" />
            <div className="relative mx-auto max-w-4xl text-center">
              <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl lg:text-6xl">
                Integrations & Workflows
              </h1>
              <p className="mx-auto max-w-3xl text-lg leading-relaxed text-neutral-600 sm:text-xl">
                Connect Char with your favorite tools. Automate repetitive tasks
                with powerful workflows. No coding required.
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
    </div>
  );
}
