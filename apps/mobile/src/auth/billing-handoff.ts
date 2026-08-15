export const MOBILE_BILLING_RETURN_URL = "anarlog://billing/refresh";

export type BillingCallback = {
  checkout: "trial" | "paid" | "canceled" | "failed" | null;
  checkoutType: "trial" | "paid" | null;
  source: "mobile" | "unknown";
};

const isCheckout = (
  value: string | null,
): value is NonNullable<BillingCallback["checkout"]> =>
  value === "trial" ||
  value === "paid" ||
  value === "canceled" ||
  value === "failed";

const isCheckoutType = (
  value: string | null,
): value is NonNullable<BillingCallback["checkoutType"]> =>
  value === "trial" || value === "paid";

export function parseBillingCallbackUrl(url: string): BillingCallback | null {
  try {
    const parsed = new URL(url);
    if (
      parsed.protocol !== "anarlog:" ||
      parsed.hostname !== "billing" ||
      parsed.pathname.replace(/\/+$/, "") !== "/refresh"
    ) {
      return null;
    }

    const checkout = parsed.searchParams.get("checkout");
    const checkoutType = parsed.searchParams.get("checkout_type");
    return {
      checkout: isCheckout(checkout) ? checkout : null,
      checkoutType: isCheckoutType(checkoutType) ? checkoutType : null,
      source:
        parsed.searchParams.get("source") === "mobile" ? "mobile" : "unknown",
    };
  } catch {
    return null;
  }
}

export async function refreshBillingEntitlement({
  refresh,
  wait = (delayMs) =>
    new Promise<void>((resolve) => setTimeout(resolve, delayMs)),
  delaysMs = [0, 1_500, 3_000, 5_000],
}: {
  refresh: () => Promise<boolean>;
  wait?: (delayMs: number) => Promise<void>;
  delaysMs?: number[];
}): Promise<boolean> {
  for (const delayMs of delaysMs) {
    if (delayMs > 0) await wait(delayMs);
    if (await refresh()) return true;
  }
  return false;
}
