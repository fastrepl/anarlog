import { createFileRoute } from "@tanstack/react-router";

import { servePublicAsset } from "../../lib/public-assets";

export const Route = createFileRoute("/api/assets/$")({
  server: {
    handlers: {
      GET: ({ request, params }) => servePublicAsset(request, params._splat),
      HEAD: ({ request, params }) => servePublicAsset(request, params._splat),
      OPTIONS: ({ request, params }) =>
        servePublicAsset(request, params._splat),
    },
  },
});
