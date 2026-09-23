import type Stripe from "stripe";

import type { BillingPeriod } from "./subscription-selection.ts";
import {
  getSyncDeviceAddonQuantity,
  selectBasePlanItem,
  selectSyncDeviceAddonItem,
  SYNC_DEVICE_ADDON_LOOKUP_KEYS,
} from "./sync-device-addon.ts";

export async function getSyncDeviceAddonPrice(
  stripe: Stripe,
  period: BillingPeriod,
) {
  const lookupKey = SYNC_DEVICE_ADDON_LOOKUP_KEYS[period];
  const prices = await stripe.prices.list({
    lookup_keys: [lookupKey],
    active: true,
    limit: 1,
  });
  const price = prices.data[0];
  if (!price)
    throw new Error(`Missing Stripe price for lookup key: ${lookupKey}`);
  return price;
}

export async function switchPlanWithAddon({
  stripe,
  subscription,
  targetPriceId,
  period,
  confirmed,
  returnUrl,
}: {
  stripe: Stripe;
  subscription: Stripe.Subscription;
  targetPriceId: string;
  period: BillingPeriod;
  confirmed: boolean;
  returnUrl: string;
}) {
  const base = selectBasePlanItem(subscription.items.data);
  const addon = selectSyncDeviceAddonItem(subscription.items.data);
  if (!base || !addon || base.id === addon.id)
    throw new Error("Subscription items are unavailable");
  const price = await getSyncDeviceAddonPrice(stripe, period);
  const items = [
    { id: base.id, price: targetPriceId, quantity: base.quantity },
    { id: addon.id, price: price.id, quantity: addon.quantity },
  ];

  if (!confirmed) {
    const invoice = await stripe.invoices.createPreview({
      subscription: subscription.id,
      subscription_details: { items, proration_behavior: "always_invoice" },
    });
    return {
      url: null,
      confirmation: {
        amountDue: invoice.amount_due,
        currency: invoice.currency,
      },
    };
  }

  // The portal cannot update multiple items. Change both atomically, leaving
  // the existing plan intact if payment fails or requires authentication.
  await stripe.subscriptions.update(subscription.id, {
    items,
    proration_behavior: "always_invoice",
    payment_behavior: "error_if_incomplete",
  });
  return { url: returnUrl, confirmation: null };
}

export async function updateDeviceAddonQuantity({
  stripe,
  subscription,
  period,
  quantity,
}: {
  stripe: Stripe;
  subscription: Stripe.Subscription;
  period: BillingPeriod;
  quantity: number;
}) {
  const addon = selectSyncDeviceAddonItem(subscription.items.data);
  const currentQuantity = getSyncDeviceAddonQuantity(subscription.items.data);
  if (currentQuantity === quantity) return;
  const proration_behavior =
    quantity > currentQuantity ? "always_invoice" : "create_prorations";
  if (quantity === 0) {
    if (addon)
      await stripe.subscriptionItems.del(addon.id, { proration_behavior });
  } else if (addon) {
    await stripe.subscriptionItems.update(addon.id, {
      quantity,
      proration_behavior,
      payment_behavior: "error_if_incomplete",
    });
  } else {
    const price = await getSyncDeviceAddonPrice(stripe, period);
    await stripe.subscriptionItems.create({
      subscription: subscription.id,
      price: price.id,
      quantity,
      proration_behavior,
      payment_behavior: "error_if_incomplete",
    });
  }
}
