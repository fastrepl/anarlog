export async function waitForBillingUpdate(
  refreshSession: () => Promise<unknown>,
  delayMs = 3000,
): Promise<void> {
  await refreshSession();
  await new Promise((r) => setTimeout(r, delayMs));
}
