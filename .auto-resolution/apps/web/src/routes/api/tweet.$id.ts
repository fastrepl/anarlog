import { createFileRoute } from "@tanstack/react-router";
import { json } from "@tanstack/react-start";
import { getTweet } from "react-tweet/api";

export const Route = createFileRoute("/api/tweet/$id")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const { id } = params;

        try {
          const tweet = await getTweet(id);
          return json({ data: tweet ?? null }, { status: tweet ? 200 : 404 });
        } catch (error) {
          return json(
            {
              error: error instanceof Error ? error.message : "bad_request",
            },
            { status: 400 },
          );
        }
      },
    },
  },
});
