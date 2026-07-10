import type { LanguageModel, ToolSet } from "ai";
import { useEffect, useMemo, useState } from "react";

import { commands as templateCommands } from "@hypr/plugin-template";

import { CustomChatTransport } from "./index";
import type { ResolvedChatContext } from "./index";

import { useLanguageModel } from "~/ai/hooks";
import type { ContextRef } from "~/chat/context/entities";
import { hydrateSessionContext } from "~/chat/context/session-context-hydrator";
import { loadHuman, loadOrganization } from "~/contacts/queries";
import { useToolRegistry } from "~/contexts/tool";
import { useConfigValue } from "~/shared/config";

export const FILE_CONTEXT_TOOL_GUIDANCE = `
Context and local-note tool guidance:
- When no meeting note context is attached and the user asks a factual question that could be answered by meeting notes, use search_sessions with the key names, topics, and date hints before answering. If the result looks relevant, answer from the returned meeting context or call read_note with the returned session id for more detail.
- When the user asks about "this note", "this meeting", "the current note", or pronouns that likely refer to the open note, use read_current_note before answering.
- When the user asks to find or search for exact wording in notes, use grep_notes. If the answer needs the full source after a match, use read_note with the returned session id.
- When the user asks about people from the current note or related meetings, use list_related_notes and then read_note as needed.
- When the user corrects note content with wording like "it's not X but Y", use apply_session_correction to update the current session summary and transcript unless they explicitly ask for one target only. Use read_current_note first when you need exact summary text. Add uncommon names, companies, products, acronyms, or jargon from the correction to dictionaryTerms so future transcription can prefer them; skip common names. If the tool reports partial, read the note or retry with the exact remaining text instead of claiming both were updated.
- Do not ask the user to open or share a meeting note until search_sessions, grep_notes, or read_note cannot find enough local context.
- Do not assume note contents from chat history when a file-backed tool can read the current source of truth.

Web search guidance:
- Use web_search for public websites, URLs, companies, products, people, news, or current facts that may be outside local notes.
- Include source URLs in the final answer when web_search results are used.
- Do not use web_search for questions that only need local notes, contacts, or calendar events.
`.trim();

export function appendFileContextToolGuidance(
  prompt: string | undefined,
): string | undefined {
  if (prompt === undefined) {
    return undefined;
  }

  if (!prompt.trim()) {
    return FILE_CONTEXT_TOOL_GUIDANCE;
  }

  return `${prompt.trim()}\n\n${FILE_CONTEXT_TOOL_GUIDANCE}`;
}

async function renderHumanContext(humanId: string): Promise<string | null> {
  const human = await loadHuman(humanId);
  if (!human) return null;
  const organization = await loadOrganization(human.organizationId);

  const name = human.name.trim() || null;
  const email = human.email.trim() || null;
  const jobTitle = human.jobTitle.trim() || null;
  const organizationName = organization?.name.trim() || null;
  const memo = human.memo.trim() || null;

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

async function renderOrganizationContext(
  organizationId: string,
): Promise<string | null> {
  const organization = await loadOrganization(organizationId);
  const name = organization?.name.trim() || null;

  return name ? `Referenced organization: ${name}` : null;
}

export function useTransport(
  modelOverride?: LanguageModel,
  extraTools?: ToolSet,
  systemPromptOverride?: string,
  userId?: string,
) {
  const registry = useToolRegistry();
  const configuredModel = useLanguageModel("chat");
  const model = modelOverride ?? configuredModel;
  const language = useConfigValue("ai_language") || "en";
  const [systemPrompt, setSystemPrompt] = useState<string | undefined>();

  useEffect(() => {
    if (systemPromptOverride) {
      setSystemPrompt(systemPromptOverride);
      return;
    }

    let stale = false;

    void (async () => {
      try {
        const result = await templateCommands.render({
          chatSystem: {
            language,
          },
        });
        if (stale) {
          return;
        }

        if (result.status === "ok") {
          setSystemPrompt(result.data);
        } else {
          setSystemPrompt("");
        }
      } catch (error) {
        console.error(error);
        if (!stale) {
          setSystemPrompt("");
        }
      }
    })();

    return () => {
      stale = true;
    };
  }, [language, systemPromptOverride]);

  const effectiveSystemPrompt = appendFileContextToolGuidance(
    systemPromptOverride ?? systemPrompt,
  );
  const isSystemPromptReady =
    typeof systemPromptOverride === "string" || systemPrompt !== undefined;

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
        if (ref.kind === "session") {
          const context = await hydrateSessionContext(ref.sessionId, userId);
          return context
            ? ({ kind: "session", context } satisfies ResolvedChatContext)
            : null;
        }

        if (ref.kind === "human") {
          const text = await renderHumanContext(ref.humanId);
          return text
            ? ({ kind: "text", text } satisfies ResolvedChatContext)
            : null;
        }

        const text = await renderOrganizationContext(ref.organizationId);
        return text
          ? ({ kind: "text", text } satisfies ResolvedChatContext)
          : null;
      },
    );
  }, [model, tools, effectiveSystemPrompt, userId]);

  return {
    transport,
    isSystemPromptReady,
  };
}
