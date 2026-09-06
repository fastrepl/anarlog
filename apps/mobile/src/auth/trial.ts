export async function enrollInTrial({
  apiUrl,
  accessToken,
  signal,
  request = fetch,
}: {
  apiUrl: string;
  accessToken: string;
  signal: AbortSignal;
  request?: typeof fetch;
}): Promise<boolean> {
  const headers = { Authorization: `Bearer ${accessToken}` };
  const base = apiUrl.replace(/\/+$/, "");
  const eligibility = await request(`${base}/subscription/can-start-trial`, {
    headers,
    signal,
    redirect: "error",
  });
  if (!eligibility.ok) throw new Error("Could not check your Pro trial.");
  const status = await eligibility.json();
  if (
    status.canStartTrial === false &&
    (status.reason === "not_eligible" || status.reason == null)
  )
    return false;
  if (
    status.canStartTrial !== true ||
    (status.reason != null && status.reason !== "eligible")
  )
    throw new Error("Could not check your Pro trial.");

  const response = await request(
    `${base}/subscription/start-trial?interval=monthly`,
    { method: "POST", headers, signal, redirect: "error" },
  );
  if (!response.ok) throw new Error("Could not start your Pro trial.");
  const result = await response.json();
  if (result.started === true) return true;
  if (result.started === false && result.reason === "not_eligible")
    return false;
  throw new Error("Could not start your Pro trial.");
}
