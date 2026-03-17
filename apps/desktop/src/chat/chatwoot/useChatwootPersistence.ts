import { useCallback, useEffect, useRef, useState } from "react";

import {
  createContact,
  createConversation,
  sendMessage,
} from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";

import { useAuth } from "~/auth";
import { env } from "~/env";

function makeClient(accessToken?: string | null) {
  const headers: Record<string, string> = {};
  if (accessToken) {
    headers.Authorization = `Bearer ${accessToken}`;
  }
  return createClient({ baseUrl: env.VITE_API_URL, headers });
}

export function useChatwootPersistence(
  userId: string | undefined,
  contactInfo?: {
    email?: string;
    name?: string;
    customAttributes?: Record<string, unknown>;
  },
) {
  const { session } = useAuth();
  const [sourceId, setSourceId] = useState<string | null>(null);
  const [pubsubToken, setPubsubToken] = useState<string | null>(null);
  const [conversationId, setConversationId] = useState<number | null>(null);
  const conversationIdRef = useRef<number | null>(null);
  const initRef = useRef(false);

  useEffect(() => {
    if (!userId || initRef.current) {
      return;
    }
    initRef.current = true;

    const client = makeClient(session?.access_token);

    createContact({
      client,
      body: {
        identifier: userId,
        email: contactInfo?.email,
        name: contactInfo?.name,
        customAttributes: contactInfo?.customAttributes,
      },
    }).then(({ data }) => {
      if (data) {
        setSourceId(data.sourceId);
        setPubsubToken(data.pubsubToken);
      }
    });
  }, [userId, session?.access_token]);

  const startConversation = useCallback(async () => {
    if (!sourceId) {
      return null;
    }

    const client = makeClient(session?.access_token);
    const { data } = await createConversation({
      client,
      body: { sourceId },
    });

    if (data) {
      const convId = data.conversationId;
      conversationIdRef.current = convId;
      setConversationId(convId);
      return convId;
    }
    return null;
  }, [sourceId, session?.access_token]);

  const persistMessage = useCallback(
    async (content: string, messageType: "incoming" | "outgoing") => {
      const convId = conversationIdRef.current;
      if (convId == null || !sourceId) {
        return;
      }

      const client = makeClient(session?.access_token);
      await sendMessage({
        client,
        path: { conversation_id: convId },
        body: {
          content,
          messageType,
          sourceId,
        },
      });
    },
    [sourceId, session?.access_token],
  );

  return {
    sourceId,
    pubsubToken,
    conversationId,
    startConversation,
    persistMessage,
    isReady: !!sourceId,
  };
}
