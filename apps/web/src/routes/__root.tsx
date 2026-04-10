import type { QueryClient } from "@tanstack/react-query";
import {
  createRootRouteWithContext,
  HeadContent,
  Scripts,
} from "@tanstack/react-router";

import { Toaster } from "@hypr/ui/components/ui/toast";

import { NotFoundDocument } from "@/components/not-found";
import { getRequestPlatform } from "@/functions/platform";
import { PlatformProvider } from "@/hooks/use-platform";
import {
  DEFAULT_OG_IMAGE_URL,
  ROOT_DESCRIPTION,
  ROOT_KEYWORDS,
  ROOT_TITLE,
} from "@/lib/seo";
import { getGitHubStatsQueryOptions } from "@/queries";
import appCss from "@/styles.css?url";

interface RouterContext {
  queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<RouterContext>()({
  loader: async ({ context }) => {
    await context.queryClient.prefetchQuery(getGitHubStatsQueryOptions());

    return {
      initialPlatform: await getRequestPlatform(),
    };
  },
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      {
        name: "viewport",
        content: "width=device-width, initial-scale=1",
      },
      { title: ROOT_TITLE },
      { name: "description", content: ROOT_DESCRIPTION },
      { name: "keywords", content: ROOT_KEYWORDS },
      { name: "ai-sitemap", content: "https://char.com/llms.txt" },
      { name: "ai-content", content: "public" },
      { property: "og:type", content: "website" },
      { property: "og:title", content: ROOT_TITLE },
      { property: "og:description", content: ROOT_DESCRIPTION },
      { property: "og:url", content: "https://char.com" },
      {
        property: "og:image",
        content: DEFAULT_OG_IMAGE_URL,
      },
      { property: "og:image:width", content: "1200" },
      { property: "og:image:height", content: "630" },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:site", content: "@getcharnotes" },
      { name: "twitter:creator", content: "@getcharnotes" },
      { name: "twitter:title", content: ROOT_TITLE },
      { name: "twitter:description", content: ROOT_DESCRIPTION },
      { name: "twitter:url", content: "https://char.com" },
      {
        name: "twitter:image",
        content: DEFAULT_OG_IMAGE_URL,
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
  const { initialPlatform } = Route.useLoaderData();

  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body>
        <PlatformProvider initialPlatform={initialPlatform}>
          {children}
        </PlatformProvider>
        <Toaster position="bottom-right" />
        <Scripts />
      </body>
    </html>
  );
}
