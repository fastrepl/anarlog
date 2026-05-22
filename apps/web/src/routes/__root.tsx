import type { QueryClient } from "@tanstack/react-query";
import {
  createRootRouteWithContext,
  HeadContent,
  Link,
  Outlet,
  Scripts,
} from "@tanstack/react-router";

import { Toaster } from "@hypr/ui/components/ui/toast";

import { WebProviders } from "@/components/web-providers";
import {
  ANARLOG_SITE_URL,
  DEFAULT_OG_IMAGE_URL,
  ROOT_DESCRIPTION,
  ROOT_KEYWORDS,
  ROOT_TITLE,
} from "@/lib/seo";
import appCss from "@/styles.css?url";

interface RouterContext {
  queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<RouterContext>()({
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
      { name: "ai-sitemap", content: `${ANARLOG_SITE_URL}/llms.txt` },
      { name: "ai-content", content: "public" },
      { name: "apple-mobile-web-app-title", content: "Anarlog" },
      { name: "theme-color", content: "#ffe09d" },
      { property: "og:type", content: "website" },
      { property: "og:title", content: ROOT_TITLE },
      { property: "og:description", content: ROOT_DESCRIPTION },
      { property: "og:url", content: ANARLOG_SITE_URL },
      {
        property: "og:image",
        content: DEFAULT_OG_IMAGE_URL,
      },
      { property: "og:image:width", content: "1200" },
      { property: "og:image:height", content: "630" },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:site", content: "@anarlog" },
      { name: "twitter:creator", content: "@anarlog" },
      { name: "twitter:title", content: ROOT_TITLE },
      { name: "twitter:description", content: ROOT_DESCRIPTION },
      { name: "twitter:url", content: ANARLOG_SITE_URL },
      {
        name: "twitter:image",
        content: DEFAULT_OG_IMAGE_URL,
      },
    ],
    // Render-blocking stylesheets are placed directly in the shell JSX
    // (RootDocument) before <HeadContent /> so the browser discovers them
    // before TanStack Router's 70+ modulepreload links. Only non-blocking
    // links belong here.
    links: [
      { rel: "icon", href: "/favicon.ico", sizes: "any" },
      {
        rel: "icon",
        href: "/favicon-32x32.png",
        type: "image/png",
        sizes: "32x32",
      },
      {
        rel: "icon",
        href: "/favicon-16x16.png",
        type: "image/png",
        sizes: "16x16",
      },
      {
        rel: "apple-touch-icon",
        href: "/apple-touch-icon.png",
        sizes: "180x180",
      },
      { rel: "manifest", href: "/manifest.json" },
    ],
  }),
  component: RootApp,
  shellComponent: RootDocument,
  notFoundComponent: NotFound,
});

function RootApp() {
  const { queryClient } = Route.useRouteContext();

  return (
    <WebProviders queryClient={queryClient}>
      <Outlet />
    </WebProviders>
  );
}

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <link rel="stylesheet" href={appCss} />
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

function NotFound() {
  return (
    <main className="flex min-h-screen items-center justify-center bg-[#f7f2e8] px-5 text-center text-[#181613]">
      <div>
        <p className="text-sm font-medium tracking-[0.18em] text-[#756b5d] uppercase">
          Not found
        </p>
        <h1 className="mt-3 text-4xl font-semibold tracking-normal">
          This page is not available.
        </h1>
        <Link
          to="/"
          className="mt-6 inline-flex rounded-full bg-[#181613] px-5 py-3 text-sm font-medium text-white"
        >
          Back to Anarlog
        </Link>
      </div>
    </main>
  );
}
