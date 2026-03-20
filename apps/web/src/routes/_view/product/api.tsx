import { createFileRoute } from "@tanstack/react-router";

import { cn } from "@hypr/utils";

import { MockWindow } from "@/components/mock-window";

export const Route = createFileRoute("/_view/product/api")({
  component: Component,
  head: () => ({
    meta: [
      { title: "API - Char" },
      {
        name: "description",
        content: "Char API for developers. Coming soon.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  return (
    <div
      className="h-[calc(100vh-65px)] bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto h-full max-w-6xl border-x border-neutral-100 bg-white">
        <div className="flex h-full items-center justify-center overflow-auto bg-[linear-gradient(to_bottom,rgba(245,245,244,0.2),white_50%,rgba(245,245,244,0.3))] px-6 py-12">
          <header className="mx-auto mb-12 max-w-4xl text-center">
            <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
              Char API
            </h1>
            <p className="mb-8 text-lg text-neutral-600 sm:text-xl">
              Build custom applications and integrations with the Char API.
            </p>

            <div className="mb-8 flex justify-center">
              <MockWindow variant="desktop">
                <div className="rounded-b-xl bg-black p-4 text-left font-mono text-sm text-green-400">
                  <div className="mb-2">
                    <span className="text-white">$</span> curl -X POST
                    https://api.hyprnote.com/v1/notes \
                  </div>
                  <div className="mb-2 ml-4">
                    -H{" "}
                    <span className="text-yellow-300">
                      "Authorization: Bearer YOUR_API_KEY"
                    </span>{" "}
                    \
                  </div>
                  <div className="mb-2 ml-4">
                    -H{" "}
                    <span className="text-yellow-300">
                      "Content-Type: application/json"
                    </span>{" "}
                    \
                  </div>
                  <div className="mb-4 ml-4">
                    -d{" "}
                    <span className="text-yellow-300">
                      '{"{"}"title": "Meeting Notes", "content": "..."{"}"}'
                    </span>
                  </div>
                  <div className="text-gray-400">
                    {"{"}
                    <div className="ml-4">"id": "note_1a2b3c4d",</div>
                    <div className="ml-4">"title": "Meeting Notes",</div>
                    <div className="ml-4">
                      "created_at": "2025-11-09T10:30:00Z"
                    </div>
                    {"}"}
                  </div>
                </div>
              </MockWindow>
            </div>

            <div>
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
          </header>
        </div>
      </div>
    </div>
  );
}
