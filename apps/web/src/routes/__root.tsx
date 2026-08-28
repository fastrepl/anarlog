import * as stylex from "@stylexjs/stylex";
import type { QueryClient } from "@tanstack/react-query";
import {
  createRootRouteWithContext,
  HeadContent,
  Link,
  Outlet,
  Scripts,
  useRouterState,
} from "@tanstack/react-router";

import { radii } from "@anlg/design-system/tokens.stylex";
import { Toaster } from "@anlg/ui/components/ui/toast";

import { WebProviders } from "@/components/web-providers";
import { isTelemetryPrivateLocation } from "@/lib/auth-route-privacy";
import {
  ANARLOG_SITE_NAME,
  ANARLOG_SITE_URL,
  DEFAULT_OG_IMAGE_URL,
  ROOT_DESCRIPTION,
  ROOT_KEYWORDS,
  ROOT_TITLE,
  getCanonicalUrl,
} from "@/lib/seo";
import appCss from "@/styles.css?url";
const styles = stylex.create({
  style1: {
    display: "flex",
    minHeight: "100vh",
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "#f7f2e8",
    paddingInline: "1.25rem",
    textAlign: "center",
    color: "#181613",
  },
  style2: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    letterSpacing: ".18em",
    color: "#756b5d",
    textTransform: "uppercase",
  },
  style3: {
    marginTop: ".75rem",
    fontSize: "2.25rem",
    lineHeight: "2.5rem",
    fontWeight: 600,
    letterSpacing: 0,
  },
  style4: {
    marginTop: "1.5rem",
    display: "inline-flex",
    borderRadius: radii.full,
    backgroundColor: "#181613",
    paddingInline: "1.25rem",
    paddingBlock: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#fff",
  },
});
interface RouterContext {
  queryClient: QueryClient;
}
const FONT_STYLESHEET =
  "https://fonts.googleapis.com/css2?family=Caveat:wght@400..700&family=Patrick+Hand&family=Reenie+Beanie&display=swap";
const FONT_STYLESHEET_ID = "anarlog-google-fonts";
export const Route = createRootRouteWithContext<RouterContext>()({
  head: () => ({
    meta: [
      {
        charSet: "utf-8",
      },
      {
        name: "viewport",
        content: "width=device-width, initial-scale=1",
      },
      {
        title: ROOT_TITLE,
      },
      {
        name: "description",
        content: ROOT_DESCRIPTION,
      },
      {
        name: "keywords",
        content: ROOT_KEYWORDS,
      },
      {
        name: "ai-sitemap",
        content: `${ANARLOG_SITE_URL}/llms.txt`,
      },
      {
        name: "ai-content",
        content: "public",
      },
      {
        name: "apple-mobile-web-app-title",
        content: "Anarlog",
      },
      {
        name: "theme-color",
        content: "#ffe09d",
      },
      {
        property: "og:type",
        content: "website",
      },
      {
        property: "og:site_name",
        content: ANARLOG_SITE_NAME,
      },
      {
        property: "og:title",
        content: ROOT_TITLE,
      },
      {
        property: "og:description",
        content: ROOT_DESCRIPTION,
      },
      {
        property: "og:url",
        content: getCanonicalUrl(),
      },
      {
        property: "og:image",
        content: DEFAULT_OG_IMAGE_URL,
      },
      {
        property: "og:image:width",
        content: "1200",
      },
      {
        property: "og:image:height",
        content: "630",
      },
      {
        name: "twitter:card",
        content: "summary_large_image",
      },
      {
        name: "twitter:site",
        content: "@anarlogapp",
      },
      {
        name: "twitter:creator",
        content: "@anarlogapp",
      },
      {
        name: "twitter:title",
        content: ROOT_TITLE,
      },
      {
        name: "twitter:description",
        content: ROOT_DESCRIPTION,
      },
      {
        name: "twitter:url",
        content: getCanonicalUrl(),
      },
      {
        name: "twitter:image",
        content: DEFAULT_OG_IMAGE_URL,
      },
    ],
    // Stylesheets are placed directly in the shell JSX (RootDocument) before
    // <HeadContent /> so the browser discovers them before TanStack Router's
    // modulepreload links. Only non-blocking links belong here.
    links: [
      {
        rel: "icon",
        href: "/favicon.ico",
        sizes: "any",
      },
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
      {
        rel: "manifest",
        href: "/manifest.json",
      },
    ],
  }),
  component: RootApp,
  shellComponent: RootDocument,
  notFoundComponent: NotFound,
});
function RootApp() {
  const { queryClient } = Route.useRouteContext();
  const telemetryEnabled = useRouterState({
    select: (state) =>
      !isTelemetryPrivateLocation(
        state.location.pathname,
        state.location.search,
      ),
  });
  return (
    <WebProviders queryClient={queryClient} telemetryEnabled={telemetryEnabled}>
      <Outlet />
    </WebProviders>
  );
}
function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link
          rel="preconnect"
          href="https://fonts.gstatic.com"
          crossOrigin="anonymous"
        />
        <link rel="preload" as="style" href={FONT_STYLESHEET} />
        <link
          id={FONT_STYLESHEET_ID}
          rel="stylesheet"
          href={FONT_STYLESHEET}
          media="print"
        />
        <script
          dangerouslySetInnerHTML={{
            __html: `(() => { const link = document.getElementById("${FONT_STYLESHEET_ID}"); if (!link) return; const apply = () => { link.media = "all"; }; if (link.sheet) apply(); else link.addEventListener("load", apply, { once: true }); })();`,
          }}
        />
        <noscript>
          <link rel="stylesheet" href={FONT_STYLESHEET} />
        </noscript>
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
    <main {...stylex.props(styles.style1)}>
      <div>
        <p {...stylex.props(styles.style2)}>Not found</p>
        <h1 {...stylex.props(styles.style3)}>This page is not available.</h1>
        <Link to="/" {...stylex.props(styles.style4)}>
          Back to Anarlog
        </Link>
      </div>
    </main>
  );
}
