export function createRefreshFetch({
  fetch,
  supabaseUrl,
  beginRefresh,
  finishRefresh,
}: {
  fetch: typeof globalThis.fetch;
  supabaseUrl: string;
  beginRefresh: () => Promise<{
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
  let inFlight: Promise<Response> | null = null;
  let inFlightBody: BodyInit | null | undefined;

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
    const refresh = async () => {
      const permit = await beginRefresh();
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
    if (
      inFlight &&
      (typeof init?.body !== "string" || init.body !== inFlightBody)
    ) {
      return refresh();
    }
    if (!inFlight) {
      inFlightBody = init?.body;
      inFlight = refresh().finally(() => {
        inFlight = null;
        inFlightBody = undefined;
      });
    }
    return (await inFlight).clone();
  };
}
