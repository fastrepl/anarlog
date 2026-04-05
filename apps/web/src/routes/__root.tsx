import type { QueryClient } from "@tanstack/react-query";
import {
  createRootRouteWithContext,
  HeadContent,
  Scripts,
} from "@tanstack/react-router";

import { Toaster } from "@hypr/ui/components/ui/toast";

import { NotFoundDocument } from "@/components/not-found";
import appCss from "@/styles.css?url";

interface RouterContext {
  queryClient: QueryClient;
}

const TITLE = "Char - Your Command Center";
const DESCRIPTION =
  "Char is a local-first command center that captures your meetings, tracks your activity, and manages your day—all on-device, all private. Daily notes that evolve with you, powered by on-device AI.";
const KEYWORDS =
  "command center, daily notes, local-first, on-device AI, privacy-first, activity capture, meeting notes, task management, personal knowledge base, AI notepad, cognitive load, founder tools";

export const Route = createRootRouteWithContext<RouterContext>()({
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      {
        name: "viewport",
        content: "width=device-width, initial-scale=1",
      },
      { title: TITLE },
      { name: "description", content: DESCRIPTION },
      { name: "keywords", content: KEYWORDS },
      { name: "ai-sitemap", content: "https://char.com/llms.txt" },
      { name: "ai-content", content: "public" },
      { property: "og:type", content: "website" },
      { property: "og:title", content: TITLE },
      { property: "og:description", content: DESCRIPTION },
      { property: "og:url", content: "https://char.com" },
      {
        property: "og:image",
        content: "/api/images/hyprnote/og-image.jpg",
      },
      { property: "og:image:width", content: "1200" },
      { property: "og:image:height", content: "630" },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:site", content: "@getcharnotes" },
      { name: "twitter:creator", content: "@getcharnotes" },
      { name: "twitter:title", content: TITLE },
      { name: "twitter:description", content: DESCRIPTION },
      { name: "twitter:url", content: "https://char.com" },
      {
        name: "twitter:image",
        content: "/api/images/hyprnote/og-image.jpg",
      },
    ],
    links: [
      { rel: "icon", href: "/favicon.svg", type: "image/svg+xml" },
      { rel: "icon", href: "/favicon.ico", sizes: "32x32" },
      { rel: "stylesheet", href: appCss },
    ],
  }),
  shellComponent: RootDocument,
  notFoundComponent: NotFoundDocument,
});

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Toaster position="bottom-right" />
        <Scripts />
      </body>
    </html>
  );
}
