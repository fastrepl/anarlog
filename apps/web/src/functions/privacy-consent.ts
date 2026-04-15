import { createServerFn } from "@tanstack/react-start";
import { getCookies, getRequestHeaders } from "@tanstack/react-start/server";

import { resolvePrivacyConsentRegion } from "@/lib/privacy-consent";

function getNetlifyGeo() {
  return (
    globalThis as typeof globalThis & {
      Netlify?: {
        context?: {
          geo?: {
            country?: { code?: string };
            subdivision?: { code?: string };
          };
        } | null;
      };
    }
  ).Netlify?.context?.geo;
}

export const getPrivacyConsentRegion = createServerFn({
  method: "GET",
}).handler(() => {
  const headers = getRequestHeaders();
  const cookies = getCookies();
  const netlifyGeo = getNetlifyGeo();

  return resolvePrivacyConsentRegion({
    countryCode:
      netlifyGeo?.country?.code ??
      headers.get("x-vercel-ip-country") ??
      headers.get("cf-ipcountry") ??
      headers.get("cloudfront-viewer-country") ??
      cookies.nf_country,
    subdivisionCode:
      netlifyGeo?.subdivision?.code ??
      headers.get("x-vercel-ip-country-region"),
  });
});
