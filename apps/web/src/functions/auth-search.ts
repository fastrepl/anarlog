import { defaultParseSearch } from "@tanstack/react-router";
import { z } from "zod";

import { flowSearchSchema } from "./desktop-flow.ts";

export const authSearchSchema = flowSearchSchema({
  redirect: z.string().optional(),
  provider: z.enum(["apple", "azure", "github", "google"]).optional(),
  view: z.enum(["email", "sso"]).optional(),
});

export function invalidAuthSearchResponse(
  request: Request,
): Response | undefined {
  const url = new URL(request.url);
  if (
    (request.method !== "GET" && request.method !== "HEAD") ||
    (url.pathname !== "/auth" && url.pathname !== "/auth/")
  ) {
    return;
  }

  if (!authSearchSchema.safeParse(defaultParseSearch(url.search)).success) {
    return new Response(
      request.method === "HEAD"
        ? null
        : "Invalid sign-in link. Please open /auth/ to sign in.",
      {
        status: 400,
        headers: {
          "Content-Type": "text/plain; charset=utf-8",
          "Cache-Control": "no-store",
          "X-Content-Type-Options": "nosniff",
        },
      },
    );
  }
}
