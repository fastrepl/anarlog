import { createFileRoute } from "@tanstack/react-router";

import { DownloadButton } from "@/components/download-button";
import { GithubStars } from "@/components/github-stars";

export const Route = createFileRoute("/_view/product/daily-notes")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Daily Notes - Char" },
      {
        name: "description",
        content:
          "Char builds a daily note from your meetings, emails, and screen activity. Review what happened, tick what's done, and hand off the rest to AI agents.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  return (
    <div className="min-h-screen overflow-x-hidden px-2 md:px-8">
      <section className="px-4 py-12 lg:py-20">
        <header className="mb-12 text-left">
          <h1 className="text-color mb-6 font-mono text-2xl tracking-wide sm:text-5xl">
            Your day, already written.
          </h1>
          <p className="text-fg-muted max-w-2xl text-lg sm:text-xl">
            Char records meetings without bots, pulls action items from your
            emails, and builds a daily note with everything you need to do. You
            review it, tick what's done, and hand off the rest to AI agents like
            Claude or Cursor.
          </p>
          <div className="mt-8 flex items-center gap-4">
            <DownloadButton />
            <GithubStars />
          </div>
        </header>
      </section>
    </div>
  );
}
