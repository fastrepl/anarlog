import { ElicitationRequestSchema, type MCPClient } from "@ai-sdk/mcp";
import { useCallback, useEffect, useRef, useState } from "react";

const ELICITATION_TIMEOUT_MS = 60_000;

type PendingElicitation = {
  message: string;
  requestedSchema: Record<string, unknown> | undefined;
  resolve: (approved: boolean) => void;
};

export function useMCPElicitation(client: MCPClient | null) {
  const [pendingElicitation, setPendingElicitation] =
    useState<PendingElicitation | null>(null);
  const pendingRef = useRef<PendingElicitation | null>(null);

  const respondToElicitation = useCallback((approved: boolean) => {
    const pending = pendingRef.current;
    pendingRef.current = null;
    setPendingElicitation(null);
    pending?.resolve(approved);
  }, []);

  useEffect(() => {
    if (!client) {
      pendingRef.current = null;
      setPendingElicitation(null);
      return;
    }

    client.onElicitationRequest(ElicitationRequestSchema, async (request) => {
      if (pendingRef.current) {
        return { action: "decline" as const };
      }

      const approved = await Promise.race([
        new Promise<boolean>((resolve) => {
          const pending: PendingElicitation = {
            message: request.params.message,
            requestedSchema: request.params.requestedSchema as
              | Record<string, unknown>
              | undefined,
            resolve,
          };
          pendingRef.current = pending;
          setPendingElicitation(pending);
        }),
        new Promise<boolean>((resolve) =>
          setTimeout(() => resolve(false), ELICITATION_TIMEOUT_MS),
        ),
      ]);

      if (!approved) {
        pendingRef.current = null;
        setPendingElicitation(null);
        return { action: "decline" as const };
      }

      return {
        action: "accept" as const,
        content: { confirmed: true },
      };
    });

    return () => {
      const pending = pendingRef.current;
      pendingRef.current = null;
      setPendingElicitation(null);
      pending?.resolve(false);
    };
  }, [client]);

  return {
    pendingElicitation,
    respondToElicitation,
  };
}
