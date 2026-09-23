import type { BillingPeriod } from "./subscription-selection.ts";

export const INCLUDED_PRO_SYNC_DEVICES = 3;
export const MAX_SYNC_DEVICE_ADDONS = 50;

// Lookup keys are the contract shared with
// supabase/migrations/*_sync_device_addons.sql, which counts these items
// toward the sync device allowance.
export const SYNC_DEVICE_ADDON_LOOKUP_KEYS = {
  monthly: "hyprnote_sync_device_addon_monthly",
  yearly: "hyprnote_sync_device_addon_yearly",
} as const satisfies Record<BillingPeriod, string>;

type PriceLike = { lookup_key?: string | null };
type ItemLike = { price: PriceLike; quantity?: number };

export function isSyncDeviceAddonPrice(price: PriceLike): boolean {
  const key = price.lookup_key;
  return (
    key === SYNC_DEVICE_ADDON_LOOKUP_KEYS.monthly ||
    key === SYNC_DEVICE_ADDON_LOOKUP_KEYS.yearly
  );
}

export function selectBasePlanItem<T extends ItemLike>(
  items: readonly T[],
): T | undefined {
  return items.find((item) => !isSyncDeviceAddonPrice(item.price)) ?? items[0];
}

export function selectSyncDeviceAddonItem<T extends ItemLike>(
  items: readonly T[],
): T | undefined {
  return items.find((item) => isSyncDeviceAddonPrice(item.price));
}

export function getSyncDeviceAddonQuantity(items: readonly ItemLike[]): number {
  return items
    .filter((item) => isSyncDeviceAddonPrice(item.price))
    .reduce((total, item) => total + (item.quantity ?? 0), 0);
}
