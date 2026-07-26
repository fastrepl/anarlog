import { createFileRoute, redirect } from "@tanstack/react-router";

import { appleIntelDownloadUrl } from "@/lib/download";

export const Route = createFileRoute("/_view/download/apple-intel")({
  beforeLoad: async () => {
    throw redirect({
      href: appleIntelDownloadUrl,
    } as any);
  },
});
