import type Stripe from "stripe";

import {
  getSyncDeviceAddonQuantity,
  selectBasePlanItem,
  selectSyncDeviceAddonItem,
} from "./sync-device-addon.ts";

export async function switchPlanWithAddon({
  stripe,
  subscription,
  targetPriceId,
  confirmed,
  returnUrl,
}: {
  stripe: Stripe;
  subscription: Stripe.Subscription;
  targetPriceId: string;
  confirmed: boolean;
  returnUrl: string;
}) {
  const base = selectBasePlanItem(subscription.items.data);
  const addon = selectSyncDeviceAddonItem(subscription.items.data);
  if (!base || !addon || base.id === addon.id)
    throw new Error("Subscription items are unavailable");
  const items = [
    { id: base.id, price: targetPriceId, quantity: base.quantity },
    { id: addon.id, deleted: true },
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
  quantity,
}: {
  stripe: Stripe;
  subscription: Stripe.Subscription;
  quantity: number;
}) {
  const addon = selectSyncDeviceAddonItem(subscription.items.data);
  const currentQuantity = getSyncDeviceAddonQuantity(subscription.items.data);
  if (quantity > currentQuantity) {
    throw new Error("Unlimited synced devices are included with Pro");
  }
  if (currentQuantity === quantity || !addon) return;
  if (quantity === 0) {
    await stripe.subscriptionItems.del(addon.id, {
      proration_behavior: "create_prorations",
    });
  } else {
    await stripe.subscriptionItems.update(addon.id, {
      quantity,
      proration_behavior: "create_prorations",
      payment_behavior: "error_if_incomplete",
    });
  }
}
