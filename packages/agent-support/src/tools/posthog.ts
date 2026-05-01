import { tool } from "@langchain/core/tools";
import type { LangGraphRunnableConfig } from "@langchain/langgraph";
import { z } from "zod";

import { posthogSpecialist } from "../specialists/posthog";

export const posthogTool = tool(
  async ({ request }: { request: string }, config: LangGraphRunnableConfig) => {
    config.writer?.({ type: "subgraph", name: "posthog", task: request });
    return posthogSpecialist.invoke({ request });
  },
  {
    name: "posthog",
    description:
      "Handle any PostHog analytics operation. Describe what you need (e.g., 'get user activity', 'query events', 'analyze funnel'). The PostHog specialist will figure out how to accomplish it.",
    schema: z.object({
      request: z
        .string()
        .describe("Natural language description of the PostHog operation"),
    }),
  },
);
