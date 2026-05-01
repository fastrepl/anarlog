import { tool } from "@langchain/core/tools";
import type { LangGraphRunnableConfig } from "@langchain/langgraph";
import { z } from "zod";

import { supabaseSpecialist } from "../specialists/supabase";

export const supabaseTool = tool(
  async ({ request }: { request: string }, config: LangGraphRunnableConfig) => {
    config.writer?.({ type: "subgraph", name: "supabase", task: request });
    return supabaseSpecialist.invoke({ request });
  },
  {
    name: "supabase",
    description:
      "Handle any Supabase-related operation. Describe what you need (e.g., 'list users', 'query orders table', 'delete user by email'). The Supabase specialist will figure out how to accomplish it.",
    schema: z.object({
      request: z
        .string()
        .describe("Natural language description of the Supabase operation"),
    }),
  },
);
