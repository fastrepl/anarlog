import { servePublicAsset } from "../lib/public-assets.ts";

export default {
  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    if (url.hostname !== "static.anarlog.so") {
      return new Response("Not found", { status: 404 });
    }
    if (url.protocol !== "https:") {
      url.protocol = "https:";
      return Response.redirect(url, 308);
    }
    if (
      /^\/og\/blog\/[a-z0-9]+(?:-[a-z0-9]+)*\/?$/.test(url.pathname) &&
      (request.method === "GET" || request.method === "HEAD")
    ) {
      const origin = new URL(`https://anarlog.so/api${url.pathname}`);
      let upstream: Response;
      try {
        upstream = await fetch(origin, { redirect: "manual" });
      } catch {
        return new Response("Preview unavailable", { status: 502 });
      }
      const contentType = upstream.headers.get("content-type");
      if (!upstream.ok || !contentType?.startsWith("image/")) {
        return new Response("Preview unavailable", {
          status: upstream.status === 404 ? 404 : 502,
        });
      }
      const headers = new Headers({
        "content-type": contentType,
        "cache-control":
          upstream.headers.get("cache-control") ?? "public, max-age=3600",
        "access-control-allow-origin": "*",
        "x-content-type-options": "nosniff",
      });
      if (request.method === "HEAD") {
        await upstream.body?.cancel();
        return new Response(null, { headers });
      }
      return new Response(upstream.body, { headers });
    }
    return servePublicAsset(request, url.pathname.slice(1));
  },
};
