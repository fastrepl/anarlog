import { createFileRoute, notFound, redirect } from "@tanstack/react-router";

import { getChangelogEntry } from "@/lib/changelog";

export const Route = createFileRoute("/changelog/$version")({
  loader: ({ params }) => {
    if (!getChangelogEntry("desktop", params.version)) throw notFound();
    throw redirect({
      to: "/changelog/$stream/$version/",
      params: { stream: "desktop", version: params.version },
      statusCode: 301,
    });
  },
});
