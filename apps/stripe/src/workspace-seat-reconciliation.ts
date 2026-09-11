import type Stripe from "stripe";

import { getCustomerOwner } from "./customer-metadata";

export type WorkspaceSeatEvent = {
  id: string;
  workspace_id: string;
  customer_id: string;
  quantity: number;
  occurred_at: Date;
};

export async function reconcileWorkspaceSeatEvent(
  event: WorkspaceSeatEvent,
  stripe: Pick<Stripe, "customers" | "subscriptions">,
) {
  const customer = await stripe.customers.retrieve(event.customer_id);
  if (customer.deleted)
    throw new Error("Workspace billing customer was deleted");
  const owner = getCustomerOwner(customer.metadata);
  if (owner?.kind !== "workspace" || owner.id !== event.workspace_id) {
    throw new Error("Workspace billing customer ownership mismatch");
  }

  const active: Stripe.Subscription[] = [];
  let startingAfter: string | undefined;
  for (;;) {
    const page = await stripe.subscriptions.list({
      customer: event.customer_id,
      status: "all",
      limit: 100,
      ...(startingAfter ? { starting_after: startingAfter } : {}),
    });
    active.push(
      ...page.data.filter((subscription) =>
        ["active", "trialing", "past_due", "unpaid"].includes(
          subscription.status,
        ),
      ),
    );
    if (active.length > 1 || !page.has_more) break;
    const last = page.data.at(-1)?.id;
    if (!last || last === startingAfter)
      throw new Error("Invalid subscription pagination");
    startingAfter = last;
  }
  if (active.length !== 1) {
    throw new Error("Expected exactly one current workspace subscription");
  }
  const subscription = active[0];
  const item = subscription.items.data[0];
  if (
    subscription.items.data.length !== 1 ||
    !item.price.recurring ||
    item.price.recurring.usage_type !== "licensed" ||
    item.price.recurring.interval_count !== 1 ||
    !["month", "year"].includes(item.price.recurring.interval) ||
    subscription.schedule
  ) {
    throw new Error("Unsupported workspace seat subscription configuration");
  }

  // A retry after Stripe succeeded but the database commit failed is a no-op.
  if (
    item.quantity === event.quantity ||
    subscription.metadata.anarlog_seat_event_id ===
      `${event.workspace_id}:${event.id}`
  )
    return;
  const occurredAt = Math.floor(event.occurred_at.getTime() / 1000);
  // Never silently reprice a historical change in a later billing period.
  // A pre-checkout event may be applied at the subscription's first period start.
  const prorationDate = Math.max(occurredAt, subscription.start_date);
  if (
    prorationDate < item.current_period_start ||
    prorationDate >= item.current_period_end
  ) {
    throw new Error(
      "Workspace seat event needs historical invoice reconciliation",
    );
  }

  // Preserve the invoiced quantity baseline until payment settles. The durable
  // queue then replays every change at its original time, with valid credits.
  if (["past_due", "unpaid"].includes(subscription.status)) {
    throw new Error("Workspace seat billing is waiting for invoice payment");
  }

  await stripe.subscriptions.update(
    subscription.id,
    {
      items: [{ id: item.id, quantity: event.quantity }],
      metadata: { anarlog_seat_event_id: `${event.workspace_id}:${event.id}` },
      proration_date: prorationDate,
      proration_behavior: "create_prorations",
    },
    { idempotencyKey: `workspace-seat-${event.workspace_id}-${event.id}` },
  );
}
