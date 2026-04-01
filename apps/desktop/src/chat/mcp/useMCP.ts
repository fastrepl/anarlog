import type { ToolSet } from "ai";
import { useEffect, useState } from "react";

import type { MCPClientConfig } from "./useMCPClient";
import { useMCPClient } from "./useMCPClient";
import { useMCPElicitation } from "./useMCPElicitation";

import type { ContextEntity } from "~/chat/context/entities";

export interface MCPConfig extends MCPClientConfig {
  enabled: boolean;
  accessToken?: string | null;
  promptName?: string;
  collectContext?: () => Promise<{
    entities: ContextEntity[];
    block: string | null;
  }>;
}

export function useMCP(config: MCPConfig) {
  const { enabled, accessToken, promptName, collectContext } = config;
  const { client, isConnected, error } = useMCPClient(
    enabled,
    config,
    accessToken,
  );
  const { pendingElicitation, respondToElicitation } =
    useMCPElicitation(client);

  const [tools, setTools] = useState<ToolSet>({});
  const [systemPrompt, setSystemPrompt] = useState<string | undefined>();
  const [contextEntities, setContextEntities] = useState<ContextEntity[]>([]);
  const [isReady, setIsReady] = useState(!enabled);

  useEffect(() => {
    if (!enabled) {
      setTools({});
      setSystemPrompt(undefined);
      setContextEntities([]);
      setIsReady(true);
      return;
    }

    if (isConnected && !client && error) {
      setTools({});
      setSystemPrompt(undefined);
      setContextEntities([]);
      setIsReady(true);
      return;
    }

    if (!isConnected || !client) {
      setIsReady(false);
      return;
    }

    let cancelled = false;

    const load = async () => {
      try {
        const [contextResult, fetchedTools, prompt] = await Promise.all([
          collectContext?.() ?? Promise.resolve({ entities: [], block: null }),
          client.tools(),
          promptName
            ? client
                .experimental_getPrompt({ name: promptName })
                .catch(() => null)
            : Promise.resolve(null),
        ]);

        if (cancelled) return;

        setContextEntities(contextResult.entities);
        setTools(fetchedTools as ToolSet);

        let mcpPrompt: string | undefined;
        if (prompt?.messages) {
          mcpPrompt = prompt.messages
            .map((m: { content: { type: string; text?: string } | string }) => {
              if (typeof m.content === "string") return m.content;
              if (m.content.type === "text" && m.content.text)
                return m.content.text;
              return "";
            })
            .filter(Boolean)
            .join("\n\n");
        }
        setSystemPrompt(
          [mcpPrompt, contextResult.block].filter(Boolean).join("\n\n") ||
            undefined,
        );
        setIsReady(true);
      } catch (error) {
        console.error("Failed to load MCP resources:", error);
        if (cancelled) return;
        setTools({});
        setSystemPrompt(undefined);
        setContextEntities([]);
        setIsReady(true);
      }
    };

    load();

    return () => {
      cancelled = true;
    };
  }, [enabled, client, isConnected, error, promptName, collectContext]);

  return {
    tools,
    systemPrompt,
    contextEntities,
    pendingElicitation,
    respondToElicitation,
    isReady,
  };
}
