import { useQuery } from "@tanstack/react-query";
import type { LanguageModel, ToolSet } from "ai";
import { useMemo } from "react";

import { commands as templateCommands } from "@hypr/plugin-template";

import { CustomChatTransport } from "./index";
import type { ResolvedChatContext } from "./index";

import { useLanguageModel } from "~/ai/hooks";
import type { ContextRef } from "~/chat/context/entities";
import { hydrateSessionContextFromFs } from "~/chat/context/session-context-hydrator";
import { useToolRegistry } from "~/contexts/tool";
import { useConfigValues } from "~/shared/config";
import * as main from "~/store/tinybase/store/main";

function renderHumanContext(
  store: ReturnType<typeof main.UI.useStore>,
  humanId: string,
): string | null {
  if (!store) {
    return null;
  }

  const human = store.getRow("humans", humanId);
  const orgId = typeof human.org_id === "string" ? human.org_id : "";
  const organization =
    orgId && store.hasRow("organizations", orgId)
      ? store.getRow("organizations", orgId)
      : {};

  const name =
    typeof human.name === "string" && human.name.trim() ? human.name : null;
  const email =
    typeof human.email === "string" && human.email.trim() ? human.email : null;
  const jobTitle =
    typeof human.job_title === "string" && human.job_title.trim()
      ? human.job_title
      : null;
  const organizationName =
    typeof organization.name === "string" && organization.name.trim()
      ? organization.name
      : null;
  const memo =
    typeof human.memo === "string" && human.memo.trim() ? human.memo : null;

  if (!name && !email) {
    return null;
  }

  const details = [
    jobTitle,
    organizationName ? `Organization: ${organizationName}` : null,
    email ? `Email: ${email}` : null,
    memo ? `Notes: ${memo}` : null,
  ].filter(Boolean);

  return [`Referenced contact: ${name ?? email}`, ...details].join("\n");
}

function renderOrganizationContext(
  store: ReturnType<typeof main.UI.useStore>,
  organizationId: string,
): string | null {
  if (!store) {
    return null;
  }

  const organization = store.getRow("organizations", organizationId);
  const name =
    typeof organization.name === "string" && organization.name.trim()
      ? organization.name
      : null;

  return name ? `Referenced organization: ${name}` : null;
}

export function useTransport(
  modelOverride?: LanguageModel,
  extraTools?: ToolSet,
  systemPromptOverride?: string,
  store?: ReturnType<typeof main.UI.useStore>,
) {
  const registry = useToolRegistry();
  const configuredModel = useLanguageModel("chat");
  const model = modelOverride ?? configuredModel;
  const {
    ai_language: language,
    chat_style_tone: styleTone,
    chat_warmth: warmth,
    chat_enthusiasm: enthusiasm,
    chat_headers_lists: headersLists,
    chat_emoji: emoji,
    chat_custom_instructions: customInstructions,
  } = useConfigValues([
    "ai_language",
    "chat_style_tone",
    "chat_warmth",
    "chat_enthusiasm",
    "chat_headers_lists",
    "chat_emoji",
    "chat_custom_instructions",
  ] as const);

  const normalizedCustomInstructions = customInstructions.trim();

  const systemPromptQuery = useQuery({
    queryKey: [
      "chat-system-prompt",
      language,
      styleTone,
      warmth,
      enthusiasm,
      headersLists,
      emoji,
      normalizedCustomInstructions,
    ],
    enabled: systemPromptOverride === undefined,
    staleTime: Infinity,
    queryFn: async () => {
      const result = await templateCommands.render({
        chatSystem: {
          language,
          styleTone,
          warmth,
          enthusiasm,
          headersLists,
          emoji,
          customInstructions: normalizedCustomInstructions,
        },
      });

      if (result.status === "ok") {
        return result.data;
      }

      return "";
    },
  });

  const effectiveSystemPrompt = systemPromptOverride ?? systemPromptQuery.data;
  const isSystemPromptReady =
    typeof systemPromptOverride === "string" ||
    systemPromptQuery.data !== undefined;

  const tools = useMemo(() => {
    const localTools = registry.getTools("chat-general");

    if (extraTools && import.meta.env.DEV) {
      for (const key of Object.keys(extraTools)) {
        if (key in localTools) {
          console.warn(
            `[ChatSession] Tool name collision: "${key}" exists in both local registry and extraTools. extraTools will take precedence.`,
          );
        }
      }
    }

    return {
      ...localTools,
      ...extraTools,
    };
  }, [registry, extraTools]);

  const transport = useMemo(() => {
    if (!model) {
      return null;
    }

    return new CustomChatTransport(
      model,
      tools,
      effectiveSystemPrompt,
      async (ref: ContextRef) => {
        if (!store) {
          return null;
        }
        if (ref.kind === "session") {
          const context = await hydrateSessionContextFromFs(
            store,
            ref.sessionId,
          );
          return context
            ? ({ kind: "session", context } satisfies ResolvedChatContext)
            : null;
        }

        if (ref.kind === "human") {
          const text = renderHumanContext(store, ref.humanId);
          return text
            ? ({ kind: "text", text } satisfies ResolvedChatContext)
            : null;
        }

        const text = renderOrganizationContext(store, ref.organizationId);
        return text
          ? ({ kind: "text", text } satisfies ResolvedChatContext)
          : null;
      },
    );
  }, [model, tools, effectiveSystemPrompt, store]);

  return {
    transport,
    isSystemPromptReady,
  };
}
