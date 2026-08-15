import { z } from "zod";

export const checkoutSourceSchema = z.enum([
  "onboarding",
  "settings",
  "trial_ended",
  "feature_gate",
  "mobile",
  "unknown",
]);

export type CheckoutSource = z.infer<typeof checkoutSourceSchema>;

export function buildBillingRefreshDeeplink({
  scheme,
  checkout,
  checkoutType,
  source,
}: {
  scheme: string;
  checkout?: "trial" | "paid" | "canceled" | "failed";
  checkoutType?: "trial" | "paid";
  source?: CheckoutSource;
}) {
  const search = new URLSearchParams();
  if (checkout) search.set("checkout", checkout);
  if (checkoutType) search.set("checkout_type", checkoutType);
  if (source) search.set("source", source);
  const query = search.toString();
  return `${scheme}://billing/refresh${query ? `?${query}` : ""}`;
}
