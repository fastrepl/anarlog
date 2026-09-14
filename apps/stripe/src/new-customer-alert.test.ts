import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import {
  sendNewCustomerAlert,
  type NewCustomerAlertDependencies,
} from "./new-customer-alert";

type Post = { webhookUrl: string; text: string };

function subscriptionCreated(
  subscription: Record<string, unknown> = {},
  livemode = true,
): Stripe.Event {
  return {
    id: "evt_subscription_created",
    type: "customer.subscription.created",
    livemode,
    data: {
      object: {
        id: "sub_new",
        customer: "cus_new",
        status: "trialing",
        items: { data: [{ price: { product: "prod_pro" } }] },
        ...subscription,
      } as unknown as Stripe.Subscription,
    },
  } as Stripe.Event;
}

function dependencies(
  posts: Post[],
  overrides: Partial<NewCustomerAlertDependencies> = {},
  customer: Record<string, unknown> = {},
): NewCustomerAlertDependencies {
  return {
    anarlogWebhookUrl: "https://hooks.example/anarlog",
    charWebhookUrl: "https://hooks.example/char",
    getCustomer: async () =>
      ({
        id: "cus_new",
        email: "new@example.com",
        metadata: { userId: "user-1" },
        ...customer,
      }) as unknown as Stripe.Customer,
    getProductName: async () => "Anarlog Pro",
    postSlackMessage: async (webhookUrl, text) => {
      posts.push({ webhookUrl, text });
    },
    ...overrides,
  };
}

describe("sendNewCustomerAlert", () => {
  it("announces an Anarlog trial in the Anarlog channel", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      subscriptionCreated(),
      dependencies(posts),
    );

    expect(result).toEqual({ product: "anarlog", subscriptionId: "sub_new" });
    expect(posts).toEqual([
      {
        webhookUrl: "https://hooks.example/anarlog",
        text: "<https://dashboard.stripe.com/customers/cus_new|new@example.com> started Pro trial",
      },
    ]);
  });

  it("announces a paid Char subscription in the Char channel", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      subscriptionCreated({ status: "active" }),
      dependencies(
        posts,
        { getProductName: async () => "Char Max" },
        { metadata: { autumn_id: "member-live-123" } },
      ),
    );

    expect(result).toEqual({ product: "char", subscriptionId: "sub_new" });
    expect(posts).toEqual([
      {
        webhookUrl: "https://hooks.example/char",
        text: "<https://dashboard.stripe.com/customers/cus_new|new@example.com> subscribed to Max",
      },
    ]);
  });

  it("links test-mode customers and falls back to the customer id", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      subscriptionCreated({}, false),
      dependencies(posts, {}, { email: null }),
    );

    expect(posts[0]?.text).toBe(
      "<https://dashboard.stripe.com/test/customers/cus_new|cus_new> started Pro trial",
    );
  });

  it("escapes Slack control characters in the email", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      subscriptionCreated(),
      dependencies(posts, {}, { email: "a<b>&c@example.com" }),
    );

    expect(posts[0]?.text).toContain("|a&lt;b&gt;&amp;c@example.com>");
  });

  it("skips a product whose channel webhook is not configured", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      subscriptionCreated(),
      dependencies(posts, { anarlogWebhookUrl: undefined }),
    );

    expect(result).toBeNull();
    expect(posts).toEqual([]);
  });

  it("skips deleted customers", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      subscriptionCreated(),
      dependencies(posts, { getCustomer: async () => null }),
    );

    expect(result).toBeNull();
    expect(posts).toEqual([]);
  });

  it("ignores other events, including customer creation", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      { ...subscriptionCreated(), type: "customer.created" } as Stripe.Event,
      dependencies(posts),
    );

    expect(result).toBeNull();
    expect(posts).toEqual([]);
  });
});
