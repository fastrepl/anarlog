import { createMiddleware } from "@tanstack/react-start";

// The router normalizes bare paths with a 307, which search engines treat as
// temporary and keep the bare URL as a canonical candidate (GSC flagged
// "Duplicate, Google chose different canonical than user"). Redirect
// permanently here before the router runs. API routes serve at bare paths
// today, so they must stay untouched.
export const trailingSlashMiddleware = createMiddleware({
  type: "request",
}).server(({ next, request, pathname, serverFnMeta }) => {
  const lastSegment = pathname.slice(pathname.lastIndexOf("/") + 1);
  const isBarePagePath =
    (request.method === "GET" || request.method === "HEAD") &&
    !serverFnMeta &&
    pathname !== "/" &&
    !pathname.endsWith("/") &&
    pathname !== "/api" &&
    !pathname.startsWith("/api/") &&
    !pathname.startsWith("/_") &&
    !lastSegment.includes(".");

  if (isBarePagePath) {
    const url = new URL(request.url);
    url.pathname = `${pathname}/`;
    return new Response(null, {
      status: 308,
      headers: { Location: url.toString() },
    });
  }

  return next();
});
