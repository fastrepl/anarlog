import { createFileRoute } from "@tanstack/react-router";

const STORAGE_BUCKETS = {
  public_images:
    "https://auth.hyprnote.com/storage/v1/object/public/public_images",
  blog: "https://auth.hyprnote.com/storage/v1/object/public/blog",
} as const;

const SAFE_SEGMENT = /^[A-Za-z0-9._+\- ]+$/;

const DEFAULT_CACHE_CONTROL = "public, max-age=31536000, immutable";

// Supabase sends `max-age=undefined` for objects uploaded without an explicit
// cacheControl, which browsers reject and treat as uncacheable. Only forward an
// upstream value when it actually parses.
function getCacheControl(upstream: string | null) {
  if (!upstream) return DEFAULT_CACHE_CONTROL;

  const maxAge = upstream.match(/max-age=([^,\s]+)/i)?.[1];
  if (maxAge && !/^\d+$/.test(maxAge)) {
    return DEFAULT_CACHE_CONTROL;
  }

  return upstream;
}

function sanitizePath(raw: string | undefined): string[] | null {
  if (!raw) return null;

  let decoded: string;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    return null;
  }

  if (decoded.startsWith("/") || decoded.includes("\\")) {
    return null;
  }

  const segments = decoded.split("/");
  if (segments.length === 0) return null;

  for (const segment of segments) {
    if (!segment) return null;
    if (segment === "." || segment === "..") return null;
    if (!SAFE_SEGMENT.test(segment)) return null;
  }

  return segments;
}

function encodePath(segments: string[]) {
  return segments.map((segment) => encodeURIComponent(segment)).join("/");
}

function getStorageUrl(segments: string[]): string | null {
  const [bucket, ...pathSegments] = segments;

  if (bucket === "blog") {
    if (pathSegments.length === 0) {
      return null;
    }

    return `${STORAGE_BUCKETS.blog}/${encodePath(pathSegments)}`;
  }

  return `${STORAGE_BUCKETS.public_images}/${encodePath(segments)}`;
}

export const Route = createFileRoute("/api/assets/$")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const sanitizedPath = sanitizePath(params._splat);

        if (!sanitizedPath) {
          return new Response("Not found", { status: 404 });
        }

        const url = getStorageUrl(sanitizedPath);
        if (!url) {
          return new Response("Not found", { status: 404 });
        }

        const response = await fetch(url);

        if (!response.ok) {
          if (response.status === 404) {
            return new Response("Not found", { status: 404 });
          }

          return new Response("Upstream service error", {
            status: 502,
          });
        }

        const contentType = response.headers.get("content-type");

        const headers: HeadersInit = {
          "Cache-Control": getCacheControl(
            response.headers.get("cache-control"),
          ),
        };
        if (contentType) {
          headers["Content-Type"] = contentType;
        }

        return new Response(response.body, {
          status: 200,
          headers,
        });
      },
    },
  },
});
