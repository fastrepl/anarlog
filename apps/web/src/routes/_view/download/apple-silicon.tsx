import { createFileRoute, redirect } from "@tanstack/react-router";

import { appleSiliconDownloadUrl } from "@/lib/download";

export const Route = createFileRoute("/_view/download/apple-silicon")({
  beforeLoad: async () => {
    throw redirect({
      href: appleSiliconDownloadUrl,
    } as any);
  },
});
