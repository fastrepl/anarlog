// Search params for sending a signed-out /app visitor to /auth and back.
// The iOS app's marker rides along so its sign-in shows no trial offer.
export function signInRedirectSearch(
  pathname: string,
  search: Record<string, unknown>,
) {
  const query = new URLSearchParams(
    search as Record<string, string>,
  ).toString();
  return {
    flow: "web" as const,
    redirect: pathname + (query ? `?${query}` : ""),
    ...(search.client === "ios" ? { client: "ios" as const } : {}),
  };
}
