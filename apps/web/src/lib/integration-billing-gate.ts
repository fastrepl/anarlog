export function getIntegrationBillingGate({
  action,
  isBillingReady,
  isVerifying,
  verificationFailed,
  verifiedIsPaid,
}: {
  action: "connect" | "reconnect" | "disconnect";
  isBillingReady: boolean;
  isVerifying: boolean;
  verificationFailed: boolean;
  verifiedIsPaid: boolean | undefined;
}) {
  if (action === "disconnect") {
    return "disconnect";
  }

  if (!isBillingReady || isVerifying) {
    return "loading";
  }

  if (verificationFailed) {
    return "retry";
  }

  if (verifiedIsPaid === undefined) {
    return "loading";
  }

  return verifiedIsPaid ? "connect" : "upgrade";
}
