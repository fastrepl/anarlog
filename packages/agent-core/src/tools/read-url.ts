import { tool } from "@langchain/core/tools";
import { z } from "zod";

import { env } from "../env";

export const readUrlTool = tool(
  async ({ url }: { url: string }) => {
    const readerUrl = `https://r.jina.ai/${url}`;
    const headers: Record<string, string> = {
      Accept: "text/plain",
    };

    if (env.JINA_API_KEY) {
      headers["Authorization"] = `Bearer ${env.JINA_API_KEY}`;
    }

    const response = await fetch(readerUrl, { headers });

    if (!response.ok) {
      return `Failed to read URL: ${response.status} ${response.statusText}`;
    }

    return response.text();
  },
  {
    name: "readUrl",
    description:
      "Read any URL and convert it to clean, LLM-friendly markdown text. Useful for reading documentation, articles, or any web page content. Uses Jina Reader API.",
    schema: z.object({
      url: z.string().url().describe("The URL to read and convert to markdown"),
    }),
  },
);
