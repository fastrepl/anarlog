export function createAuthFetch(
  baseFetch: typeof fetch,
  getAccessToken: () => string | undefined | Promise<string | undefined>,
  refreshAccessToken?: () => Promise<string | undefined>,
): typeof fetch {
  return async (input, init) => {
    const headers = new Headers(init?.headers);
    const token = await getAccessToken();
    if (token) {
      headers.set("Authorization", `Bearer ${token}`);
    }
    const response = await baseFetch(input, { ...init, headers });
    if (response.status !== 401 || !refreshAccessToken) {
      return response;
    }

    const refreshed = await refreshAccessToken();
    if (!refreshed || refreshed === token) {
      return response;
    }

    const retryHeaders = new Headers(headers);
    retryHeaders.set("Authorization", `Bearer ${refreshed}`);
    return baseFetch(input, { ...init, headers: retryHeaders });
  };
}
