import { tool } from "@langchain/core/tools";
import { z } from "zod";

import { understandAnarlogRepo } from "../modal/understand";

export const understandAnarlogRepoTool = tool(
  async ({ request }: { request: string }) => {
    const result = await understandAnarlogRepo(request);
    const lines = [
      `success: ${result.success}`,
      `executionTimeMs: ${result.executionTimeMs}`,
      `report:\n${result.report}`,
    ];
    return lines.join("\n");
  },
  {
    name: "understandAnarlogRepo",
    description:
      "Analyze and understand the Anarlog codebase using Claude CLI. Use this for questions about code structure, architecture, implementation details, or finding specific code. This tool is read-only and cannot make modifications.",
    schema: z.object({
      request: z
        .string()
        .describe(
          "The question or request about the Anarlog codebase to investigate",
        ),
    }),
  },
);
