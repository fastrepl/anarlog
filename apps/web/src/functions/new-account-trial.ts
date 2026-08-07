type TrialRequest = (accessToken: string) => Promise<{
  data?: {
    started: boolean;
    reason?: "started" | "not_eligible" | null;
  };
  error?: unknown;
}>;

async function requestTrial(accessToken: string) {
  const [{ startTrial }, { createClient }, { env }] = await Promise.all([
    import("@anlg/api-client"),
    import("@anlg/api-client/client"),
    import("../env.ts"),
  ]);
  const client = createClient({
    baseUrl: env.VITE_API_URL,
    headers: { Authorization: `Bearer ${accessToken}` },
  });

  return startTrial({
    client,
    query: { interval: "monthly" },
  });
}

export async function ensureNewAccountTrial(
  accessToken: string,
  request: TrialRequest = requestTrial,
) {
  const { data, error } = await request(accessToken);

  if (error) {
    throw error;
  }

  if (data?.started) {
    return "started" as const;
  }

  if (data?.reason === "not_eligible") {
    return "not_eligible" as const;
  }

  throw new Error("New account trial returned an invalid response");
}
