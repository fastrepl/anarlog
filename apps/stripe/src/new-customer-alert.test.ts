import { describe, expect, it } from "bun:test";
import type Stripe from "stripe";

import {
  sendNewCustomerAlert,
  type NewCustomerAlertDependencies,
} from "./new-customer-alert";

type Post = { webhookUrl: string; text: string };

const LINK = "<https://dashboard.stripe.com/customers/cus_new|new@example.com>";

function stripeEvent(
  type: string,
  object: Record<string, unknown>,
  data: Record<string, unknown> = {},
): Stripe.Event {
  return {
    id: "evt_1",
    type,
    livemode: true,
    data: { object, ...data },
  } as unknown as Stripe.Event;
}

function customer(
  metadata: Record<string, string>,
  overrides: Record<string, unknown> = {},
) {
  return { id: "cus_new", email: "new@example.com", metadata, ...overrides };
}

function subscription(status: string) {
  return {
    id: "sub_new",
    customer: "cus_new",
    status,
    items: { data: [{ price: { product: "prod_1" } }] },
  };
}

function dependencies(
  posts: Post[],
  overrides: Partial<NewCustomerAlertDependencies> = {},
  customerMetadata: Record<string, string> = { userId: "user-1" },
): NewCustomerAlertDependencies {
  return {
    anarlogWebhookUrl: "https://hooks.example/anarlog",
    charWebhookUrl: "https://hooks.example/char",
    getCustomer: async () =>
      customer(customerMetadata) as unknown as Stripe.Customer,
    getProductName: async () => "Anarlog Pro",
    postSlackMessage: async (webhookUrl, text) => {
      posts.push({ webhookUrl, text });
    },
    ...overrides,
  };
}

describe("sendNewCustomerAlert", () => {
  it("announces an Anarlog signup when its Stripe customer is created", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      stripeEvent("customer.created", customer({ userId: "user-1" })),
      dependencies(posts),
    );

    expect(result).toEqual({ product: "anarlog", kind: "signup" });
    expect(posts).toEqual([
      {
        webhookUrl: "https://hooks.example/anarlog",
        text: `${LINK} signed up to Anarlog`,
      },
    ]);
  });

  it("does not announce workspace or Char customers as signups", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      stripeEvent("customer.created", customer({ workspaceId: "ws-1" })),
      dependencies(posts),
    );
    await sendNewCustomerAlert(
      stripeEvent("customer.created", customer({ autumn_id: "member-1" })),
      dependencies(posts),
    );
    await sendNewCustomerAlert(
      stripeEvent(
        "customer.created",
        customer({ autumn_id: "member-1", userId: "user-1" }),
      ),
      dependencies(posts),
    );

    expect(posts).toEqual([]);
  });

  it("announces a subscription that starts paid", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      stripeEvent("customer.subscription.created", subscription("active")),
      dependencies(posts),
    );

    expect(result).toEqual({ product: "anarlog", kind: "plan" });
    expect(posts).toEqual([
      {
        webhookUrl: "https://hooks.example/anarlog",
        text: `${LINK} started Pro plan`,
      },
    ]);
  });

  it("stays quiet when a trial starts", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      stripeEvent("customer.subscription.created", subscription("trialing")),
      dependencies(posts),
    );

    expect(result).toBeNull();
    expect(posts).toEqual([]);
  });

  it("announces a Char trial converting to paid in the Char channel", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      stripeEvent("customer.subscription.updated", subscription("active"), {
        previous_attributes: { status: "trialing" },
      }),
      dependencies(
        posts,
        { getProductName: async () => "Char Max" },
        { autumn_id: "member-1" },
      ),
    );

    expect(result).toEqual({ product: "char", kind: "plan" });
    expect(posts).toEqual([
      {
        webhookUrl: "https://hooks.example/char",
        text: `${LINK} started Max plan`,
      },
    ]);
  });

  it("announces a subscription whose first payment completes", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      stripeEvent("customer.subscription.updated", subscription("active"), {
        previous_attributes: { status: "incomplete" },
      }),
      dependencies(posts),
    );

    expect(posts).toEqual([
      {
        webhookUrl: "https://hooks.example/anarlog",
        text: `${LINK} started Pro plan`,
      },
    ]);
  });

  it("ignores updates that are not a first paid start", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      stripeEvent("customer.subscription.updated", subscription("active"), {
        previous_attributes: { status: "past_due" },
      }),
      dependencies(posts),
    );
    await sendNewCustomerAlert(
      stripeEvent("customer.subscription.updated", subscription("active"), {
        previous_attributes: { cancel_at_period_end: true },
      }),
      dependencies(posts),
    );
    await sendNewCustomerAlert(
      stripeEvent("customer.subscription.updated", subscription("canceled"), {
        previous_attributes: { status: "trialing" },
      }),
      dependencies(posts),
    );

    expect(posts).toEqual([]);
  });

  it("escapes Slack control characters and links test-mode customers", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      {
        ...stripeEvent(
          "customer.created",
          customer({ userId: "user-1" }, { email: "a<b>&c@example.com" }),
        ),
        livemode: false,
      } as Stripe.Event,
      dependencies(posts),
    );
    await sendNewCustomerAlert(
      stripeEvent("customer.subscription.created", subscription("active")),
      dependencies(posts, { getProductName: async () => "Anarlog <Pro>" }),
    );

    expect(posts.map((post) => post.text)).toEqual([
      "<https://dashboard.stripe.com/test/customers/cus_new|a&lt;b&gt;&amp;c@example.com> signed up to Anarlog",
      `${LINK} started &lt;Pro&gt; plan`,
    ]);
  });

  it("links by customer id when the email is missing", async () => {
    const posts: Post[] = [];

    await sendNewCustomerAlert(
      stripeEvent(
        "customer.created",
        customer({ userId: "user-1" }, { email: null }),
      ),
      dependencies(posts),
    );

    expect(posts[0]?.text).toBe(
      "<https://dashboard.stripe.com/customers/cus_new|cus_new> signed up to Anarlog",
    );
  });

  it("skips a channel whose webhook is not configured", async () => {
    const posts: Post[] = [];

    const result = await sendNewCustomerAlert(
      stripeEvent("customer.created", customer({ userId: "user-1" })),
      dependencies(posts, { anarlogWebhookUrl: undefined }),
    );

    expect(result).toBeNull();
    expect(posts).toEqual([]);
  });
});
