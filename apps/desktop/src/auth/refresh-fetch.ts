export function createRefreshFetch({
  fetch,
  supabaseUrl,
  beginRefresh,
  finishRefresh,
}: {
  fetch: typeof globalThis.fetch;
  supabaseUrl: string;
  beginRefresh: (refreshToken: string) => Promise<{
    leaseId: number | null;
    retryAfterMs: number;
  }>;
  finishRefresh: (
    leaseId: number,
    status: number | null,
    retryAfterMs: number | null,
  ) => Promise<unknown>;
}): typeof globalThis.fetch {
  const tokenUrl = new URL(
    "auth/v1/token",
    `${supabaseUrl.replace(/\/+$/, "")}/`,
  );
  const inFlight = new Map<string, Promise<Response>>();

  return async (input, init) => {
    const url = new URL(
      input instanceof Request ? input.url : input.toString(),
    );
    const method =
      init?.method ?? (input instanceof Request ? input.method : "GET");
    if (
      method.toUpperCase() !== "POST" ||
      url.origin !== tokenUrl.origin ||
      url.pathname !== tokenUrl.pathname ||
      url.searchParams.get("grant_type") !== "refresh_token"
    ) {
      return fetch(input, init);
    }
    const body = await new Request(
      input instanceof Request ? input.clone() : input,
      init,
    ).text();
    let credential = body;
    try {
      const parsed = JSON.parse(body);
      if (typeof parsed.refresh_token === "string")
        credential = parsed.refresh_token;
    } catch {
      // Malformed requests still share a cooldown for their exact payload.
    }
    const refresh = async () => {
      const permit = await beginRefresh(credential);
      if (permit.leaseId === null) {
        return Response.json(
          {
            code: "over_request_rate_limit",
            message: "Auth refresh is waiting to retry",
          },
          {
            status: 429,
            headers: {
              "Retry-After": String(Math.ceil(permit.retryAfterMs / 1000)),
            },
          },
        );
      }
      let response: Response | null = null;
      const controller = new AbortController();
      const signal =
        init?.signal ?? (input instanceof Request ? input.signal : null);
      const abort = () => controller.abort(signal?.reason);
      signal?.addEventListener("abort", abort, { once: true });
      if (signal?.aborted) abort();
      const timeout = setTimeout(() => controller.abort(), 45_000);
      try {
        const upstream = await fetch(input, {
          ...init,
          signal: controller.signal,
        });
        response = new Response(await upstream.arrayBuffer(), {
          status: upstream.status,
          statusText: upstream.statusText,
          headers: upstream.headers,
        });
        return response;
      } finally {
        clearTimeout(timeout);
        signal?.removeEventListener("abort", abort);
        const retryAfter = response?.headers.get("Retry-After");
        const seconds =
          retryAfter === null || retryAfter === undefined
            ? NaN
            : Number(retryAfter);
        const delay = Number.isFinite(seconds)
          ? seconds * 1000
          : Date.parse(retryAfter ?? "") - Date.now();
        await finishRefresh(
          permit.leaseId,
          response?.status ?? null,
          Number.isFinite(delay) && delay >= 0 ? Math.ceil(delay) : null,
        ).catch(() => {
          // A release failure must not discard the server's rotated refresh token.
          console.warn("[auth] refresh gate release failed");
        });
      }
    };
    let pending = inFlight.get(credential);
    if (!pending) {
      pending = refresh().finally(() => {
        inFlight.delete(credential);
      });
      inFlight.set(credential, pending);
    }
    return (await pending).clone();
  };
}
