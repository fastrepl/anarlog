const BUCKETS = {
  images:
    "https://ijoptyyjrfqwaqhyxkxj.supabase.co/storage/v1/object/public/public_images",
  blog: "https://ijoptyyjrfqwaqhyxkxj.supabase.co/storage/v1/object/public/blog",
};

const REQUEST_HEADERS = [
  "range",
  "if-range",
  "if-none-match",
  "if-modified-since",
];
const RESPONSE_HEADERS = [
  "content-type",
  "content-length",
  "content-range",
  "accept-ranges",
  "etag",
  "last-modified",
];

const publicAssetRequestHeaders = (incoming: Headers) => {
  const headers = new Headers();
  for (const name of REQUEST_HEADERS) {
    const value = incoming.get(name);
    if (value) headers.set(name, value);
  }
  return headers;
};

export async function servePublicAsset(
  request: Request,
  path: string | undefined,
): Promise<Response> {
  const headers = new Headers({
    "access-control-allow-origin": "*",
    "access-control-allow-methods": "GET, HEAD, OPTIONS",
    "access-control-allow-headers":
      "Range, If-Range, If-None-Match, If-Modified-Since",
    "access-control-expose-headers": RESPONSE_HEADERS.join(", "),
    "x-content-type-options": "nosniff",
  });
  if (request.method === "OPTIONS")
    return new Response(null, { status: 204, headers });
  if (request.method !== "GET" && request.method !== "HEAD") {
    headers.set("allow", "GET, HEAD, OPTIONS");
    return new Response("Method not allowed", { status: 405, headers });
  }

  let decoded: string;
  try {
    decoded = decodeURIComponent(path ?? "");
  } catch {
    return new Response("Not found", { status: 404, headers });
  }
  const segments = decoded.split("/");
  if (
    segments.some(
      (segment) =>
        !segment ||
        segment === "." ||
        segment === ".." ||
        !/^[A-Za-z0-9._+\- \[\]]+$/.test(segment),
    )
  ) {
    return new Response("Not found", { status: 404, headers });
  }
  const isBlog = segments[0] === "blog";
  const objectPath = isBlog ? segments.slice(1) : segments;
  if (!objectPath.length)
    return new Response("Not found", { status: 404, headers });
  const url = `${isBlog ? BUCKETS.blog : BUCKETS.images}/${objectPath.map(encodeURIComponent).join("/")}`;

  let upstream: Response;
  try {
    upstream = await fetch(url, {
      method: request.method,
      headers: publicAssetRequestHeaders(request.headers),
      redirect: "manual",
    });
  } catch {
    return new Response("Upstream service error", { status: 502, headers });
  }
  if (!upstream.ok && upstream.status !== 304 && upstream.status !== 416) {
    return new Response(
      upstream.status === 404 ? "Not found" : "Upstream service error",
      {
        status: upstream.status === 404 ? 404 : 502,
        headers,
      },
    );
  }
  for (const name of RESPONSE_HEADERS) {
    const value = upstream.headers.get(name);
    if (value) headers.set(name, value);
  }
  const cacheControl = upstream.headers.get("cache-control");
  const maxAge = cacheControl?.match(/max-age=([^,\s]+)/i)?.[1];
  headers.set(
    "cache-control",
    segments[0] === "desktop" && /^[a-f0-9]{64}$/.test(segments[1] ?? "")
      ? "public, max-age=31536000, immutable"
      : cacheControl && maxAge && /^\d+$/.test(maxAge)
        ? cacheControl
        : "public, max-age=3600",
  );
  if (upstream.status === 416) headers.set("cache-control", "no-store");
  return new Response(
    request.method === "HEAD" || upstream.status === 304 ? null : upstream.body,
    {
      status: upstream.status,
      headers,
    },
  );
}
