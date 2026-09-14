import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import {
  sendNewCustomerAlert,
  type NewCustomerAlertDependencies,
} from "./new-customer-alert";

type Post = { webhookUrl: string; text: string };

function customerCreated(
  customer: Record<string, unknown> = {},
  livemode = true,
): Stripe.Event {
  return {
    id: "evt_customer_created",
    type: "customer.created",
    livemode,
    data: {
      object: {
        id: "cus_new",
        email: "new@example.com",
        metadata: { userId: "user-1" },
        ...customer,
      } as unknown as Stripe.Customer,
    },
  } as Stripe.Event;
}

function dependencies(
  posts: Post[],
  overrides: Partial<NewCustomerAlertDependencies> = {},
): NewCustomerAlertDependencies {
  return {
    anarlogWebhookUrl: "https://hooks.example/anarlog",
    charWebhookUrl: "https://hooks.example/char",
    postSlackMessage: async (webhookUrl, text) => {
      posts.push({ webhookUrl, text });
    },
    ...overrides,
  };
}

describe("sendNewCustomerAlert", () => {
  it("posts Anarlog customers to the Anarlog channel", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      customerCreated(),
      dependencies(posts),
    );

    expect(result).toEqual({ product: "anarlog", customerId: "cus_new" });
    expect(posts).toEqual([
      {
        webhookUrl: "https://hooks.example/anarlog",
        text: "New Anarlog customer: new@example.com\n<https://dashboard.stripe.com/customers/cus_new|View in Stripe>",
      },
    ]);
  });

  it("posts customers Char bills through Autumn to the Char channel", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      customerCreated({
        metadata: {
          autumn_id: "member-live-123",
          autumn_internal_id: "cus_autumn_123",
        },
      }),
      dependencies(posts),
    );

    expect(result).toEqual({ product: "char", customerId: "cus_new" });
    expect(posts).toEqual([
      {
        webhookUrl: "https://hooks.example/char",
        text: "New Char customer: new@example.com\n<https://dashboard.stripe.com/customers/cus_new|View in Stripe>",
      },
    ]);
  });

  it("labels workspace customers as Anarlog Team", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      customerCreated({ metadata: { workspaceId: "workspace-1" } }),
      dependencies(posts),
    );

    expect(posts[0]?.text).toStartWith("New Anarlog Team customer:");
  });

  it("links test-mode customers and notes a missing email", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      customerCreated({ email: null }, false),
      dependencies(posts),
    );

    expect(posts[0]?.text).toBe(
      "New Anarlog customer: (no email)\n<https://dashboard.stripe.com/test/customers/cus_new|View in Stripe>",
    );
  });

  it("escapes Slack control characters in the email", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      customerCreated({ email: "a<b>&c@example.com" }),
      dependencies(posts),
    );

    expect(posts[0]?.text).toStartWith(
      "New Anarlog customer: a&lt;b&gt;&amp;c@example.com\n",
    );
  });

  it("skips a product whose channel webhook is not configured", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      customerCreated({ metadata: { autumn_id: "member-live-123" } }),
      dependencies(posts, { charWebhookUrl: undefined }),
    );

    expect(result).toBeNull();
    expect(posts).toEqual([]);
  });

  it("ignores other events", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      { ...customerCreated(), type: "customer.updated" } as Stripe.Event,
      dependencies(posts),
    );

    expect(result).toBeNull();
    expect(posts).toEqual([]);
  });
});
