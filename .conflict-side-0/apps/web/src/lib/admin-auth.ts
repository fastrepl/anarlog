const GITHUB_REAUTH_ERROR_MARKERS = [
  "bad credentials",
  "github token not configured",
  "github api error: 401",
];

type ErrorResponse = {
  error?: string;
};

export class AdminSignInRedirectError extends Error {
  constructor() {
    super("Redirecting to sign in");
    this.name = "AdminSignInRedirectError";
  }
}

export function isAdminSignInRedirectError(error: unknown) {
  return error instanceof AdminSignInRedirectError;
}

export function isGitHubReauthErrorMessage(error?: string | null) {
  if (!error) {
    return false;
  }

  const normalizedError = error.toLowerCase();

  return GITHUB_REAUTH_ERROR_MARKERS.some((marker) =>
    normalizedError.includes(marker),
  );
}

export function redirectAdminToSignIn(redirectPath?: string) {
  if (typeof window === "undefined") {
    throw new AdminSignInRedirectError();
  }

  const redirect =
    redirectPath ||
    `${window.location.pathname}${window.location.search}${window.location.hash}`;
  const params = new URLSearchParams({
    flow: "web",
    provider: "github",
    redirect,
    rra: "true",
  });

  window.location.assign(`/auth/?${params.toString()}`);
  throw new AdminSignInRedirectError();
}

export async function fetchAdminJson<T>(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  fallbackError: string,
) {
  const response = await fetch(input, init);
  const data = (await response.json().catch(() => null)) as
    | T
    | ErrorResponse
    | null;

  const error =
    data && typeof data === "object" && "error" in data
      ? data.error
      : undefined;

  if (response.status === 401 || isGitHubReauthErrorMessage(error)) {
    redirectAdminToSignIn();
  }

  if (!response.ok) {
    throw new Error(error || fallbackError);
  }

  return data as T;
}
