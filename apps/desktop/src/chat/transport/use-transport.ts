import type { LanguageModel, ToolSet } from "ai";
import { useEffect, useMemo, useState } from "react";

import { commands as templateCommands } from "@anlg/plugin-template";

import { CustomChatTransport } from "./index";
import type { ResolvedChatContext } from "./index";

import { useLanguageModel } from "~/ai/hooks";
import type { ContextRef } from "~/chat/context/entities";
import { renderFolderContext } from "~/chat/context/folder-context";
import { hydrateSessionContext } from "~/chat/context/session-context-hydrator";
import { loadHuman, loadOrganization } from "~/contacts/queries";
import { useToolRegistry } from "~/contexts/tool";
import { useConfigValue } from "~/shared/config";

export const MEETING_CONTEXT_TOOL_GUIDANCE = `
Context and local meeting tool guidance:
- Use list_meetings for recent meetings, title or ID lookup, pagination, and exact recurring-series filtering. Never guess a meeting ID.
- Use search_meetings for open-ended questions about topics, people, decisions, or date ranges across meeting content. Use search_meeting_content when the user needs exact wording from notes or transcripts.
- After resolving an ID, use get_meeting for the canonical note, summaries, participants, and action items. Use get_meeting_transcript separately for bounded transcript pages, following pagination.next_offset only when more context is needed.
- Use get_recurring_meeting_history for meetings in the same recurring series. Use find_related_meetings only for broader relationships such as shared participants or nearby dates.
- When the user refers to the current meeting, prefer the attached meeting context. Do not fetch it again unless the task needs newer structured data.
- When folder context is attached, prefer the notes listed in that folder. Search and content tools stay scoped to that folder. Use read_folder_material for syllabus or other folder files listed in that context.
- When the user asks to prepare for a meeting, create an agenda, organize talking points, or add drafted content before or during a meeting, call edit_memo with the complete replacement markdown so they can review and apply it. Preserve relevant existing memo content. Use edit_memo even when the memo is empty; do not use edit_summary for meeting preparation.
- When the user asks to rewrite, revise, refocus, shorten, or restructure an existing summary, call edit_summary with the complete replacement markdown so they can review and apply it. Do not return the rewrite only as a fenced markdown block.
- Use edit_summary only for existing generated post-meeting summaries. Use apply_session_correction for narrow exact old-to-new corrections and edit_summary for broader summary rewrites. Only return a draft without calling edit_memo or edit_summary when the user explicitly asks not to change the meeting content or no target session can be resolved.
- When the user corrects note content with wording like "it's not X but Y", use apply_session_correction to update the current session summary, visible session title, and transcript unless they explicitly ask for one target only. Add uncommon names, companies, products, acronyms, or jargon from the correction to dictionaryTerms so future transcription and summaries can prefer them; skip common names. If the tool reports partial, use get_meeting or retry with the exact remaining text instead of claiming both were updated.
- When the user asks to move a recording, transcript, or notes onto a different existing meeting, resolve both meeting IDs with list_meetings or search_meetings, then call move_meeting_contents. Default the source to the current meeting when they are looking at the misplaced recording. Do not guess IDs. If the target already has a recording or transcript, explain that and stop.
- Do not ask the user to open or share a meeting until list_meetings, search_meetings, search_meeting_content, and get_meeting cannot find enough local context.
- Use typed meeting tools instead of constructing shell commands, crawling files, or accessing SQLite directly.
- Do not assume meeting contents from chat history when a typed tool can read the current source of truth.

Web search guidance:
- Use web_search for public websites, URLs, companies, products, people, news, or current facts that may be outside local notes.
- Include source URLs in the final answer when web_search results are used.
- Do not use web_search for questions that only need local notes, contacts, or calendar events.
`.trim();

export function appendMeetingContextToolGuidance(
  prompt: string | undefined,
): string | undefined {
  if (prompt === undefined) {
    return undefined;
  }

  if (!prompt.trim()) {
    return MEETING_CONTEXT_TOOL_GUIDANCE;
  }

  return `${prompt.trim()}\n\n${MEETING_CONTEXT_TOOL_GUIDANCE}`;
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

  const effectiveSystemPrompt = appendMeetingContextToolGuidance(
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

        if (ref.kind === "folder") {
          const text = await renderFolderContext(ref.folderId);
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
