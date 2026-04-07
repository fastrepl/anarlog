import { createFileRoute } from "@tanstack/react-router";

const STORAGE_BUCKETS = {
  public_images:
    "https://auth.hyprnote.com/storage/v1/object/public/public_images",
  blog: "https://auth.hyprnote.com/storage/v1/object/public/blog",
} as const;
const LEGACY_BUCKET_PREFIX = "_bucket";

const SAFE_SEGMENT = /^[A-Za-z0-9._+\- ]+$/;

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

function getStorageUrls(segments: string[]): string[] {
  if (segments[0] === LEGACY_BUCKET_PREFIX) {
    const [_, bucket, ...pathSegments] = segments;
    const storageBaseUrl =
      bucket && bucket in STORAGE_BUCKETS
        ? STORAGE_BUCKETS[bucket as keyof typeof STORAGE_BUCKETS]
        : null;

    if (!storageBaseUrl || pathSegments.length === 0) {
      return [];
    }

    return [`${storageBaseUrl}/${encodePath(pathSegments)}`];
  }

  const [bucket, ...pathSegments] = segments;

  if (bucket === "blog") {
    if (pathSegments.length === 0) {
      return [];
    }

    return [
      `${STORAGE_BUCKETS.blog}/${encodePath(pathSegments)}`,
      `${STORAGE_BUCKETS.public_images}/${encodePath(segments)}`,
    ];
  }

  if (bucket === "public_images") {
    if (pathSegments.length === 0) {
      return [];
    }

    return [`${STORAGE_BUCKETS.public_images}/${encodePath(pathSegments)}`];
  }

  return [`${STORAGE_BUCKETS.public_images}/${encodePath(segments)}`];
}

export const Route = createFileRoute("/api/assets/$")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const sanitizedPath = sanitizePath(params._splat);

        if (!sanitizedPath) {
          return new Response("Not found", { status: 404 });
        }

        const urls = getStorageUrls(sanitizedPath);
        if (urls.length === 0) {
          return new Response("Not found", { status: 404 });
        }

        let response: Response | null = null;
        for (const url of urls) {
          const candidate = await fetch(url);

          if (candidate.ok) {
            response = candidate;
            break;
          }

          if (candidate.status !== 404) {
            return new Response("Upstream service error", {
              status: 502,
            });
          }
        }

        if (!response) {
          return new Response("Not found", { status: 404 });
        }

        const contentType = response.headers.get("content-type");
        const cacheControl = response.headers.get("cache-control");

        const headers: HeadersInit = {};
        if (contentType) {
          headers["Content-Type"] = contentType;
        }
        if (cacheControl) {
          headers["Cache-Control"] = cacheControl;
        } else {
          headers["Cache-Control"] = "public, max-age=31536000, immutable";
        }

        return new Response(response.body, {
          status: 200,
          headers,
        });
      },
    },
  },
});
