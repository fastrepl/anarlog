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
    return servePublicAsset(request, url.pathname.slice(1));
  },
};
