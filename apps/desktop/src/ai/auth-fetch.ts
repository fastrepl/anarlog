export function createAuthFetch(
  baseFetch: typeof fetch,
  getAccessToken: () => string | undefined,
): typeof fetch {
  return async (input, init) => {
    const headers = new Headers(init?.headers);
    const token = getAccessToken();
    if (token) {
      headers.set("Authorization", `Bearer ${token}`);
    }
    return baseFetch(input, { ...init, headers });
  };
}
