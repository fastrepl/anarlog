import { getWorkspaceShareSlug } from "../lib/workspace-share-host.ts";

const APP_ORIGIN = "https://anarlog.netlify.app";
const PLATFORM_HOSTS = new Set([
  "api.anarlog.so",
  "desktop.anarlog.so",
  "docs.anarlog.so",
  "models.anarlog.so",
  "www.anarlog.so",
]);

export const createWorkspaceShareOriginRequest = (request: Request) => {
  const incomingUrl = new URL(request.url);
  if (getWorkspaceShareSlug(incomingUrl.hostname) === null) return null;

  const originUrl = new URL(
    incomingUrl.pathname + incomingUrl.search,
    APP_ORIGIN,
  );
  const originRequest = new Request(originUrl, request);
  originRequest.headers.set("x-forwarded-host", incomingUrl.host);
  originRequest.headers.set("x-forwarded-proto", "https");
  return originRequest;
};

export default {
  async fetch(request: Request): Promise<Response> {
    const incomingUrl = new URL(request.url);
    const originRequest = createWorkspaceShareOriginRequest(request);

    if (originRequest === null) {
      if (PLATFORM_HOSTS.has(incomingUrl.hostname)) return fetch(request);
      return new Response("Not found", { status: 404 });
    }

    if (incomingUrl.protocol !== "https:") {
      incomingUrl.protocol = "https:";
      return Response.redirect(incomingUrl, 308);
    }

    return fetch(originRequest);
  },
};
