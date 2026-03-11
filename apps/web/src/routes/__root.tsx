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

const TITLE = "Char - AI notepad for private meetings";
const DESCRIPTION =
  "Char is a private, on-device AI notepad that enhances your own notes—without bots, cloud recording, or meeting intrusion. Stay engaged, build your personal knowledge base, and export to tools like Notion on your terms.";
const KEYWORDS =
  "AI notepad, privacy-first AI, on-device AI, local AI, edge AI, meeting notes, personal knowledge base, AI notetaking, AI notetaker, Argmax, Deepgram, secure transcription, notepad app, notetaking app";

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
    links: [{ rel: "stylesheet", href: appCss }],
  }),
  scripts: ({ matches }) => {
    if (import.meta.env.DEV) {
      return [];
    }

    const isAdminRoute = matches.some((match) =>
      match.pathname.startsWith("/admin"),
    );
    if (isAdminRoute) {
      return [];
    }

    return [
      {
        id: "ze-snippet",
        src: "https://static.zdassets.com/ekr/snippet.js?key=15949e47-ed5a-4e52-846e-200dd0b8f4b9",
      },
    ];
  },
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
