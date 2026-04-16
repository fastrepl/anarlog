import type { QueryClient } from "@tanstack/react-query";
import {
  createRootRouteWithContext,
  HeadContent,
  Outlet,
  Scripts,
} from "@tanstack/react-router";

import { Toaster } from "@hypr/ui/components/ui/toast";

import { ConsentAwareProviders } from "@/components/consent-aware-providers";
import { NotFoundDocument } from "@/components/not-found";
import { PrivacyConsentProvider } from "@/components/privacy-consent";
import { getPrivacyConsentRegion } from "@/functions/privacy-consent";
import {
  DEFAULT_OG_IMAGE_URL,
  ROOT_DESCRIPTION,
  ROOT_KEYWORDS,
  ROOT_TITLE,
} from "@/lib/seo";
import appCss from "@/styles.css?url";

interface RouterContext {
  queryClient: QueryClient;
}

const FONT_STYLESHEETS = [
  "https://fonts.googleapis.com/css2?family=Geist:wght@100..900&family=Geist+Mono:wght@100..900&family=Instrument+Serif:ital@1&family=Lora:wght@400;500;600;700&display=swap",
  "https://fonts.googleapis.com/css2?family=Fraunces:ital,opsz,wght@0,9..144,100..900;1,9..144,100..900&display=swap",
] as const;

const MICROSOFT_CLARITY_SCRIPT = `(function(c,l,a,r,i,t,y){try{if(l.location&&l.location.pathname.indexOf("/admin")===0){return;}var raw=c.localStorage&&c.localStorage.getItem("char_web_tracking_consent_v1");var consent=raw?JSON.parse(raw):null;var analytics=!!(consent&&consent.analytics===true);if(c.navigator&&c.navigator.globalPrivacyControl){analytics=false;}if(!analytics){return;}}catch(e){return;}c[a]=c[a]||function(){(c[a].q=c[a].q||[]).push(arguments)};c[a]("consentv2",{ad_Storage:"denied",analytics_Storage:"granted"});t=l.createElement(r);t.id="microsoft-clarity-script";t.async=1;t.src="https://www.clarity.ms/tag/"+i;y=l.getElementsByTagName(r)[0];y.parentNode.insertBefore(t,y);})(window, document, "clarity", "script", "wcjttoibok");`;

export const Route = createRootRouteWithContext<RouterContext>()({
  loader: async () => ({
    privacyConsentRegion: await getPrivacyConsentRegion(),
  }),
  staleTime: 60 * 60 * 1000,
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
    // Render-blocking stylesheets are placed directly in the shell JSX
    // (RootDocument) before <HeadContent /> so the browser discovers them
    // before TanStack Router's 70+ modulepreload links. Only non-blocking
    // links belong here.
    links: [
      { rel: "icon", href: "/favicon.svg", type: "image/svg+xml" },
      { rel: "icon", href: "/favicon.ico", sizes: "32x32" },
    ],
    scripts: [
      {
        type: "text/javascript",
        children: MICROSOFT_CLARITY_SCRIPT,
      },
    ],
  }),
  component: RootApp,
  shellComponent: RootDocument,
  notFoundComponent: NotFoundDocument,
});

function RootApp() {
  const { queryClient } = Route.useRouteContext();
  const { privacyConsentRegion } = Route.useLoaderData();

  return (
    <PrivacyConsentProvider region={privacyConsentRegion}>
      <ConsentAwareProviders queryClient={queryClient}>
        <Outlet />
      </ConsentAwareProviders>
    </PrivacyConsentProvider>
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
        {FONT_STYLESHEETS.map((href) => (
          <link key={href} rel="stylesheet" href={href} />
        ))}
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
