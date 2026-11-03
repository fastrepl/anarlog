type Env = Cloudflare.Env;

interface ObjectKeyGrant {
  key: string;
}

const GRANT_PATH = "/sync/database-backups/worker-object-key";
const KEY_PATTERN =
  /^database-backups\/[0-9a-f-]{36}\/latest\.anarlogdb\.enc$/u;

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === "/health") return json({ ok: true });

    const method = request.method;
    if (
      !["/database-backups/upload", "/database-backups/download"].includes(
        url.pathname,
      ) ||
      (url.pathname.endsWith("/upload") && method !== "PUT") ||
      (url.pathname.endsWith("/download") && method !== "GET")
    )
      return json({ error: "not_found" }, 404);

    const authorization = request.headers.get("Authorization");
    if (!authorization?.startsWith("Bearer "))
      return json({ error: "unauthorized" }, 401);

    const grantResponse = await fetch(new URL(GRANT_PATH, env.API_ORIGIN), {
      method: "GET",
      headers: { Authorization: authorization, Accept: "application/json" },
    });
    if (!grantResponse.ok)
      return new Response(grantResponse.body, {
        status: grantResponse.status,
        headers: { "Cache-Control": "no-store" },
      });

    let grant: ObjectKeyGrant;
    try {
      grant = (await grantResponse.json()) as ObjectKeyGrant;
    } catch {
      return json({ error: "invalid_grant" }, 502);
    }
    if (typeof grant.key !== "string" || !KEY_PATTERN.test(grant.key))
      return json({ error: "invalid_grant" }, 502);

    if (url.pathname.endsWith("/upload")) {
      if (!request.body) return json({ error: "empty_body" }, 400);
      try {
        await env.BACKUPS.put(grant.key, request.body, {
          httpMetadata: { contentType: "application/octet-stream" },
        });
      } catch {
        console.error("r2_database_backup_upload_failed");
        return json({ error: "storage_unavailable" }, 502);
      }
      return new Response(null, { status: 204, headers: noStoreHeaders() });
    }

    let object: R2ObjectBody | null;
    try {
      object = await env.BACKUPS.get(grant.key);
    } catch {
      console.error("r2_database_backup_download_failed");
      return json({ error: "storage_unavailable" }, 502);
    }
    if (!object) return json({ error: "backup_not_found" }, 404);
    return new Response(object.body, {
      headers: {
        ...noStoreHeaders(),
        "Content-Type": "application/octet-stream",
        "Content-Length": String(object.size),
      },
    });
  },
};

function json(body: unknown, status = 200): Response {
  return Response.json(body, { status, headers: noStoreHeaders() });
}

function noStoreHeaders(): HeadersInit {
  return {
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  };
}
