import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { useEffect, useRef } from "react";

import { createClient } from "@hypr/api-client/client";

import { useAuth } from "~/auth";
import { env } from "~/env";

export function useChatwootEvents({
  pubsubToken,
  conversationId,
  onAgentMessage,
}: {
  pubsubToken: string | null;
  conversationId: number | null;
  onAgentMessage: (content: string, senderName: string) => void;
}) {
  const { session } = useAuth();
  const onAgentMessageRef = useRef(onAgentMessage);
  onAgentMessageRef.current = onAgentMessage;

  useEffect(() => {
    if (!pubsubToken || conversationId == null || !session?.access_token) {
      return;
    }

    const abortController = new AbortController();

    const client = createClient({ baseUrl: env.VITE_API_URL });
    const url = client.buildUrl({
      url: "/support/chatwoot/conversations/{conversation_id}/events",
      path: { conversation_id: conversationId },
      query: { pubsub_token: pubsubToken },
    });

    (async () => {
      try {
        const response = await tauriFetch(url, {
          method: "GET",
          headers: {
            Accept: "text/event-stream",
            Authorization: `Bearer ${session.access_token}`,
          },
          signal: abortController.signal,
        });

        if (!response.ok || !response.body) {
          return;
        }

        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const parts = buffer.split("\n\n");
          buffer = parts.pop() ?? "";

          for (const part of parts) {
            const dataLine = part
              .split("\n")
              .find((line) => line.startsWith("data: "));
            if (!dataLine) continue;

            try {
              const payload = JSON.parse(dataLine.slice(6));
              if (payload.content) {
                onAgentMessageRef.current(
                  payload.content,
                  payload.senderName ?? "Agent",
                );
              }
            } catch {}
          }
        }
      } catch (e) {
        if (!abortController.signal.aborted) {
          console.error("Chatwoot events stream error:", e);
        }
      }
    })();

    return () => {
      abortController.abort();
    };
  }, [pubsubToken, conversationId, session?.access_token]);
}
