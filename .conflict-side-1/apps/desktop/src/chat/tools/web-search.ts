import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { tool } from "ai";
import { z } from "zod";

import type { ToolDependencies, WebSearchResponse } from "./types";

import { env } from "~/env";

const webSearchInputSchema = z.object({
  query: z.string().min(1).describe("Search query for public web information."),
  includeDomains: z
    .array(z.string().min(1))
    .max(5)
    .optional()
    .describe(
      "Optional domains to search within, for example ['char.com']. Use only when the user names a specific site or domain.",
    ),
  excludeDomains: z
    .array(z.string().min(1))
    .max(5)
    .optional()
    .describe("Optional domains to exclude from the search results."),
  limit: z
    .number()
    .int()
    .min(1)
    .max(10)
    .optional()
    .describe("Maximum number of web results to return."),
});

export type WebSearchInput = z.infer<typeof webSearchInputSchema>;

export async function runWebSearch(
  params: WebSearchInput,
  deps: Pick<ToolDependencies, "getAuthHeaders" | "fetch">,
): Promise<WebSearchResponse> {
  const headers = deps.getAuthHeaders();
  if (!headers) {
    return {
      status: "error",
      message: "Sign in to use web search.",
      query: params.query,
      results: [],
    };
  }

  const requestHeaders = new Headers(headers);
  requestHeaders.set("Content-Type", "application/json");

  const response = await (deps.fetch ?? tauriFetch)(
    new URL("/research/search", env.VITE_API_URL).toString(),
    {
      method: "POST",
      headers: requestHeaders,
      body: JSON.stringify({
        query: params.query,
        numResults: params.limit,
        includeDomains: params.includeDomains,
        excludeDomains: params.excludeDomains,
      }),
    },
  );

  if (!response.ok) {
    return {
      status: "error",
      message: `Web search failed with HTTP ${response.status}.`,
      query: params.query,
      results: [],
    };
  }

  return {
    status: "ok",
    ...(await response.json()),
  };
}

export const buildWebSearchTool = (deps: ToolDependencies) =>
  tool({
    description: `
Search the public web for current or external information.
Use this for questions about public websites, URLs, companies, products, people, news, or facts that may not be in local notes.
Return source URLs in the final answer when web results are used.
Do not use this when the user is asking only about local notes, meetings, contacts, or calendar events.
`.trim(),
    inputSchema: webSearchInputSchema,
    execute: (params) => runWebSearch(params, deps),
  });
