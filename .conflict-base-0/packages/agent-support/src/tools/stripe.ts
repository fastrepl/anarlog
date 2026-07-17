import { tool } from "@langchain/core/tools";
import type { LangGraphRunnableConfig } from "@langchain/langgraph";
import { z } from "zod";

import { stripeSpecialist } from "../specialists/stripe";

export const stripeTool = tool(
  async ({ request }: { request: string }, config: LangGraphRunnableConfig) => {
    config.writer?.({ type: "subgraph", name: "stripe", task: request });
    return stripeSpecialist.invoke({ request });
  },
  {
    name: "stripe",
    description:
      "Handle any Stripe-related operation. Describe what you need (e.g., 'look up customer by email', 'list active subscriptions', 'cancel subscription'). The Stripe specialist will figure out how to accomplish it.",
    schema: z.object({
      request: z
        .string()
        .describe("Natural language description of the Stripe operation"),
    }),
  },
);
