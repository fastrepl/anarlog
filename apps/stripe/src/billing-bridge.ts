import type Stripe from "stripe";

import {
  getCustomerIdentityMetadata,
  getCustomerOwner,
} from "./customer-metadata";
import { getWorkspaceBillingUpdate } from "./workspace-billing";

const CUSTOMER_EVENTS: Stripe.Event.Type[] = [
  "checkout.session.completed",
  "customer.created",
  "customer.updated",
  "customer.subscription.created",
  "customer.subscription.updated",
  "customer.subscription.deleted",
];

type BillingBridgeDependencies = {
  getCustomer: (customerId: string) => Promise<Stripe.Customer | null>;
  updateCustomerMetadata: (
    customerId: string,
    metadata: Record<string, string>,
  ) => Promise<void>;
  assignProfileCustomer: (
    userId: string,
    customerId: string,
  ) => Promise<string | null | undefined>;
  deleteCustomer: (customerId: string) => Promise<void>;
  syncWorkspaceCustomer: (update: {
    workspaceId: string;
    customerId: string;
    seatLimit: number | null;
    updateSeatLimit: boolean;
  }) => Promise<string | null | undefined>;
};

export async function syncBillingBridge(
  event: Stripe.Event,
  dependencies?: BillingBridgeDependencies,
) {
  if (!isCustomerEvent(event.type)) {
    return;
  }

  const customerId = getCustomerId(event.data.object);

  if (!customerId) {
    return;
  }

  const activeDependencies =
    dependencies ?? (await createDefaultDependencies());
  const customer = await activeDependencies.getCustomer(customerId);

  if (!customer) {
    return;
  }

  const owner = getCustomerOwner(customer.metadata);

  if (!owner) {
    return;
  }

  if (owner.kind === "workspace") {
    const update = getWorkspaceBillingUpdate(event);
    const assignedCustomerId = await activeDependencies.syncWorkspaceCustomer({
      workspaceId: owner.id,
      customerId,
      ...update,
    });
    if (assignedCustomerId && assignedCustomerId !== customerId) {
      throw new Error("Workspace Stripe customer assignment conflict");
    }
    return;
  }

  const userId = owner.id;

  const identityMetadata = getCustomerIdentityMetadata(
    customer.metadata,
    userId,
  );
  if (identityMetadata) {
    await activeDependencies.updateCustomerMetadata(
      customerId,
      identityMetadata,
    );
  }

  const assignedCustomerId = await activeDependencies.assignProfileCustomer(
    userId,
    customerId,
  );
  if (assignedCustomerId !== customerId) {
    await activeDependencies.deleteCustomer(customerId);
  }
}

const isCustomerEvent = (eventType: string) =>
  CUSTOMER_EVENTS.includes(eventType as Stripe.Event.Type);

export const getCustomerId = (
  eventObject: Stripe.Event.Data.Object,
): string | null => {
  const obj = eventObject as {
    customer?: string | { id: string };
    id?: string;
  };

  if (typeof obj.customer === "string") {
    return obj.customer;
  }

  if (obj.customer && typeof obj.customer === "object") {
    return obj.customer.id;
  }

  if (obj.id?.startsWith("cus_")) {
    return obj.id;
  }

  return null;
};

export const getStripeCustomer = async (customerId: string) => {
  const { stripe } = await import("./integration/stripe");
  const customer = await stripe.customers.retrieve(customerId);

  if (isDeletedCustomer(customer)) {
    return null;
  }

  return customer;
};

const isDeletedCustomer = (
  customer: Stripe.Customer | Stripe.DeletedCustomer,
): customer is Stripe.DeletedCustomer =>
  "deleted" in customer && customer.deleted === true;

export const getUserIdFromCustomer = (
  customer: Stripe.Customer,
): string | null => {
  const owner = getCustomerOwner(customer.metadata);
  return owner?.kind === "user" ? owner.id : null;
};

async function createDefaultDependencies(): Promise<BillingBridgeDependencies> {
  const [{ stripe }, { supabaseAdmin }] = await Promise.all([
    import("./integration/stripe"),
    import("./integration/supabase"),
  ]);

  return {
    async getCustomer(customerId) {
      const customer = await stripe.customers.retrieve(customerId);
      return isDeletedCustomer(customer) ? null : customer;
    },
    async updateCustomerMetadata(customerId, metadata) {
      await stripe.customers.update(customerId, { metadata });
    },
    async assignProfileCustomer(userId, customerId) {
      const { data, error } = await supabaseAdmin.rpc(
        "assign_profile_stripe_customer",
        {
          p_owner_user_id: userId,
          p_stripe_customer_id: customerId,
        },
      );

      if (!error) {
        return data?.[0]?.assigned_customer_id as string | null | undefined;
      }
      if (error.code !== "PGRST202") {
        throw error;
      }

      const { error: updateError } = await supabaseAdmin
        .from("profiles")
        .update({ stripe_customer_id: customerId })
        .eq("id", userId)
        .is("stripe_customer_id", null);
      if (updateError) {
        throw updateError;
      }

      const { data: profile, error: profileError } = await supabaseAdmin
        .from("profiles")
        .select("stripe_customer_id")
        .eq("id", userId)
        .single();
      if (profileError) {
        throw profileError;
      }
      return profile.stripe_customer_id as string | null;
    },
    async deleteCustomer(customerId) {
      await stripe.customers.del(customerId);
    },
    async syncWorkspaceCustomer(update) {
      const { data, error } = await supabaseAdmin.rpc(
        "sync_workspace_stripe_billing",
        {
          p_workspace_id: update.workspaceId,
          p_stripe_customer_id: update.customerId,
          p_seat_limit: update.seatLimit,
          p_update_seat_limit: update.updateSeatLimit,
        },
      );
      if (error) {
        throw error;
      }
      return data?.[0]?.assigned_customer_id as string | null | undefined;
    },
  };
}
