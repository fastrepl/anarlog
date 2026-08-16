export function addNangoSessionHandoff(url: string, sessionToken: string) {
  const handoffUrl = new URL(url);
  handoffUrl.searchParams.set("handoff", "nango");
  // The fragment stays out of HTTP requests and referrers; the web app removes
  // it from browser history as soon as it captures the scoped credential.
  handoffUrl.hash = new URLSearchParams({
    session_token: sessionToken,
  }).toString();
  return handoffUrl.toString();
}
