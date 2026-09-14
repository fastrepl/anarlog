export function startBillingApi(binary: string, port: number) {
  const child = Bun.spawn([binary], {
    env: {
      ...Bun.env,
      PORT: String(port),
      ANARLOG_SERVICE: "billing",
      ANARLOG_BILLING_WEBHOOKS: "true",
    },
    stdout: "inherit",
    stderr: "inherit",
  });
  return child;
}

export async function drainBillingRuntime(
  api: Pick<Bun.Subprocess, "kill" | "exited" | "exitCode">,
  stopWebhooks: () => Promise<void>,
) {
  // The API may still be forwarding an accepted webhook to the local listener.
  // Keep that listener alive until all API requests have completed.
  if (api.exitCode === null) api.kill("SIGUSR1");
  const exitCode = await api.exited;
  await stopWebhooks();
  if (exitCode !== 0)
    throw new Error(`Billing API exited with code ${exitCode}`);
}
