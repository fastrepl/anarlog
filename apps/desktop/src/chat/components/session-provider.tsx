import { Chat, useChat } from "@ai-sdk/react";
import { useLingui } from "@lingui/react/macro";
import type { ChatStatus, ChatTransport, LanguageModel, ToolSet } from "ai";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { dedupeByKey, type ContextRef } from "~/chat/context/entities";
import {
  type DisplayEntity,
  useChatContextPipeline,
} from "~/chat/context/use-chat-context-pipeline";
import {
  createChatCloudsyncActivityController,
  guardChatTransport,
  type GuardedChatPreflight,
} from "~/chat/store/cloudsync-activity";
import {
  consumeFailedChatGroupCreate,
  hasPendingChatPersist,
  waitForPendingChatPersists,
} from "~/chat/store/pending-persists";
import {
  buildPersistedChatMessage,
  getVisibleChatMessages,
  shouldPersistFinishedMessage,
} from "~/chat/store/persisted-messages";
import {
  deleteChatMessage,
  deleteChatMessagesExcept,
  getChatMessageGroupId,
  replaceChatMessage,
  upsertChatMessage,
  usePersistedChatMessages,
} from "~/chat/store/queries";
import { stripEphemeralToolContext } from "~/chat/tools/strip-ephemeral-tool-context";
import { useTransport } from "~/chat/transport/use-transport";
import type {
  ChatMessageSender,
  ChatSendOptions,
  AnlgUIMessage,
} from "~/chat/types";
import { flushDatabaseWritesByPrefix } from "~/db/write-queue";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { useOwnerUserId } from "~/shared/owner-user";
import { id } from "~/shared/utils";

export type ChatSessionRenderProps = {
  sessionId: string;
  messages: AnlgUIMessage[];
  setMessages: (
    msgs: AnlgUIMessage[] | ((prev: AnlgUIMessage[]) => AnlgUIMessage[]),
  ) => void;
  sendMessage: ChatMessageSender;
  regenerate: () => void;
  stop: () => void;
  status: ChatStatus;
  error?: Error;
  contextEntities: DisplayEntity[];
  pendingRefs: ContextRef[];
  onRemoveContextEntity: (key: string) => void;
  onAddContextEntity: (ref: ContextRef) => void;
  onDraftContextRefsChange: (refs: ContextRef[]) => void;
  isSystemPromptReady: boolean;
};

type ChatTransportPreflight = NonNullable<ChatSendOptions["beforeSend"]>;

interface ChatSessionProps {
  sessionId: string;
  chatGroupId?: string;
  currentSessionId?: string;
  hasAvailableTranscript?: boolean;
  isBatchTranscriptionPending?: boolean;
  modelOverride?: LanguageModel;
  extraTools?: ToolSet;
  systemPromptOverride?: string;
  unstyled?: boolean;
  children: (props: ChatSessionRenderProps) => ReactNode;
}

function areMessagesEqual(a: AnlgUIMessage[], b: AnlgUIMessage[]) {
  if (a.length !== b.length) {
    return false;
  }

  return a.every((message, index) => {
    const other = b[index];
    return (
      message.id === other?.id &&
      message.role === other.role &&
      JSON.stringify(message.parts) === JSON.stringify(other.parts) &&
      JSON.stringify(message.metadata ?? {}) ===
        JSON.stringify(other.metadata ?? {})
    );
  });
}

export function ChatSession(props: ChatSessionProps) {
  return <ChatSessionLifecycle key={props.sessionId} {...props} />;
}

function ChatSessionLifecycle({
  sessionId,
  chatGroupId,
  currentSessionId,
  hasAvailableTranscript = false,
  isBatchTranscriptionPending = false,
  modelOverride,
  extraTools,
  systemPromptOverride,
  unstyled = false,
  children,
}: ChatSessionProps) {
  const { t } = useLingui();
  const ownerUserId = useOwnerUserId();
  const persistedMessages = usePersistedChatMessages(chatGroupId);

  const [pendingManualRefs, setPendingManualRefs] = useState<ContextRef[]>([]);
  const [pendingDraftRefs, setPendingDraftRefs] = useState<ContextRef[]>([]);
  const latestChatGroupIdRef = useRef(chatGroupId);
  const latestUserIdRef = useRef(ownerUserId);
  const initialMessagesRef = useRef<AnlgUIMessage[]>([]);
  const submittedChatGroupIdsRef = useRef(new Map<string, string>());
  const pendingTransportPreflightsRef = useRef(
    new Map<string, GuardedChatPreflight[]>(),
  );
  const pendingRegenerationTombstonesRef = useRef(
    new Map<
      string,
      {
        chatGroupId: string;
        assistantMessageId: string;
      }
    >(),
  );
  const pendingFinishedChatPersistsRef = useRef(
    new Map<string, Promise<void>>(),
  );
  const acceptFinishedChatPersistenceRef = useRef(true);
  const regenerateRequestInFlightRef = useRef(false);
  const chatCloudsyncActivityRef = useRef<ReturnType<
    typeof createChatCloudsyncActivityController
  > | null>(null);
  chatCloudsyncActivityRef.current ??= createChatCloudsyncActivityController();
  const chatCloudsyncActivity = chatCloudsyncActivityRef.current;
  const takeTransportPreflight = useCallback((logicalKey: string) => {
    const queue = pendingTransportPreflightsRef.current.get(logicalKey);
    const preflight = queue?.shift();
    if (queue?.length === 0) {
      pendingTransportPreflightsRef.current.delete(logicalKey);
    }
    return preflight;
  }, []);
  const removeTransportPreflight = useCallback(
    (logicalKey: string, preflight: ChatTransportPreflight) => {
      const queue = pendingTransportPreflightsRef.current.get(logicalKey);
      const index =
        queue?.findIndex((candidate) => candidate.run === preflight) ?? -1;
      if (queue && index !== -1) {
        queue.splice(index, 1);
      }
      if (queue?.length === 0) {
        pendingTransportPreflightsRef.current.delete(logicalKey);
      }
    },
    [],
  );

  latestChatGroupIdRef.current = chatGroupId;
  latestUserIdRef.current = ownerUserId;

  const onAddContextEntity = useCallback((ref: ContextRef) => {
    setPendingManualRefs((prev) =>
      prev.some((r) => r.key === ref.key) ? prev : [...prev, ref],
    );
  }, []);

  const onRemoveContextEntity = useCallback((key: string) => {
    setPendingManualRefs((prev) => prev.filter((r) => r.key !== key));
    setPendingDraftRefs((prev) => prev.filter((r) => r.key !== key));
  }, []);

  const onDraftContextRefsChange = useCallback((refs: ContextRef[]) => {
    setPendingDraftRefs(refs);
  }, []);

  useEffect(() => {
    setPendingManualRefs([]);
    setPendingDraftRefs([]);
    pendingRegenerationTombstonesRef.current.clear();
  }, [sessionId, chatGroupId]);

  const { transport, isSystemPromptReady } = useTransport(
    modelOverride,
    extraTools,
    systemPromptOverride,
    ownerUserId || undefined,
  );

  const persistedVisibleMessages = useMemo(
    () => getVisibleChatMessages(persistedMessages),
    [persistedMessages],
  );
  initialMessagesRef.current = persistedVisibleMessages;

  const chat = useMemo(
    () =>
      new Chat<AnlgUIMessage>({
        id: sessionId,
        messages: initialMessagesRef.current,
        transport: guardChatTransport(
          transport ?? unavailableChatTransport,
          chatCloudsyncActivity,
          { beforeSend: takeTransportPreflight },
        ),
        onFinish: ({ message, messages, isAbort, isError }) => {
          const currentUserId = latestUserIdRef.current;
          const messageIndex = messages.findIndex((m) => m.id === message.id);
          const lastMessageIndex =
            messageIndex === -1 ? messages.length - 1 : messageIndex - 1;
          let submittedUserMessage: AnlgUIMessage | undefined;
          for (let i = lastMessageIndex; i >= 0; i--) {
            if (messages[i].role === "user") {
              submittedUserMessage = messages[i];
              break;
            }
          }
          const submittedChatGroupId = submittedUserMessage
            ? submittedChatGroupIdsRef.current.get(submittedUserMessage.id)
            : undefined;
          if (submittedUserMessage) {
            submittedChatGroupIdsRef.current.delete(submittedUserMessage.id);
          }
          const regenerationTarget = submittedUserMessage
            ? pendingRegenerationTombstonesRef.current.get(
                submittedUserMessage.id,
              )
            : undefined;
          const advanceRegenerationTarget = (assistantMessageId: string) => {
            if (
              submittedUserMessage &&
              regenerationTarget &&
              pendingRegenerationTombstonesRef.current.get(
                submittedUserMessage.id,
              ) === regenerationTarget
            ) {
              pendingRegenerationTombstonesRef.current.set(
                submittedUserMessage.id,
                {
                  chatGroupId: regenerationTarget.chatGroupId,
                  assistantMessageId,
                },
              );
            }
          };
          const sanitizedParts = stripEphemeralToolContext(message.parts);
          const sanitizedMessage =
            sanitizedParts === message.parts
              ? message
              : { ...message, parts: sanitizedParts };
          const retainPriorRegeneration =
            regenerationTarget &&
            (isError || !shouldPersistFinishedMessage(sanitizedMessage));
          const persistAssistant =
            !isAbort &&
            acceptFinishedChatPersistenceRef.current &&
            !retainPriorRegeneration;

          if (!currentUserId || (!submittedUserMessage && !persistAssistant)) {
            if (submittedUserMessage) {
              chatCloudsyncActivity.finish(submittedUserMessage.id);
            }
            return;
          }

          const finishedPersist = chatCloudsyncActivity.runWithLease(
            submittedUserMessage?.id ?? sanitizedMessage.id,
            async () => {
              // Outbound user writes may still be retrying; settle them first
              // so the lookup below reflects the final truth and a failed
              // persist can be repaired instead of orphaning the reply.
              const awaitedChatGroupId =
                submittedChatGroupId ?? latestChatGroupIdRef.current;
              if (awaitedChatGroupId) {
                await waitForPendingChatPersists(awaitedChatGroupId);
              }

              let persistedChatGroupId: string | null = null;
              if (submittedUserMessage) {
                try {
                  persistedChatGroupId = await getChatMessageGroupId(
                    submittedUserMessage.id,
                  );
                } catch (error) {
                  console.error(
                    "Failed to resolve the persisted chat message group",
                    error,
                  );
                }
              }
              const targetChatGroupId =
                submittedChatGroupId ??
                persistedChatGroupId ??
                latestChatGroupIdRef.current;
              if (!targetChatGroupId) {
                return;
              }

              // The group row was never created; persisting into it would
              // produce orphaned rows that never appear in history.
              if (consumeFailedChatGroupCreate(targetChatGroupId)) {
                return;
              }

              // If the outbound persist failed, the assistant row would land
              // with no matching user row and reconciliation would wipe the
              // turn — repair the user message before persisting the reply.
              if (submittedUserMessage && !persistedChatGroupId) {
                await upsertChatMessage(
                  buildPersistedChatMessage({
                    message: submittedUserMessage,
                    chatGroupId: targetChatGroupId,
                    ownerUserId: currentUserId,
                    status: "ready",
                  }),
                );
              }

              if (!persistAssistant) {
                return;
              }

              if (!shouldPersistFinishedMessage(sanitizedMessage)) {
                await deleteChatMessage(targetChatGroupId, sanitizedMessage.id);
                return;
              }

              const persistedMessage = buildPersistedChatMessage({
                message: sanitizedMessage,
                chatGroupId: targetChatGroupId,
                ownerUserId: currentUserId,
                status: "ready",
              });
              if (regenerationTarget) {
                if (regenerationTarget.chatGroupId !== targetChatGroupId) {
                  throw new Error("Regenerated chat message group changed");
                }
                await replaceChatMessage({
                  message: persistedMessage,
                  previousMessageId: regenerationTarget.assistantMessageId,
                });
                advanceRegenerationTarget(sanitizedMessage.id);
                return;
              }
              await upsertChatMessage(persistedMessage);
            },
          );
          void finishedPersist.catch((error) => {
            console.error("Failed to persist finished chat message", error);
          });
          if (submittedUserMessage) {
            const userMessageId = submittedUserMessage.id;
            pendingFinishedChatPersistsRef.current.set(
              userMessageId,
              finishedPersist,
            );
            void finishedPersist
              .catch(() => undefined)
              .finally(() => {
                if (
                  pendingFinishedChatPersistsRef.current.get(userMessageId) ===
                  finishedPersist
                ) {
                  pendingFinishedChatPersistsRef.current.delete(userMessageId);
                }
              });
          }
        },
      }),
    [chatCloudsyncActivity, sessionId, takeTransportPreflight, transport],
  );

  const {
    messages,
    sendMessage: chatSendMessage,
    regenerate: chatRegenerate,
    stop: chatStop,
    status,
    error,
    setMessages: chatSetMessages,
  } = useChat<AnlgUIMessage>({ chat });
  const latestChatStopRef = useRef(chatStop);
  latestChatStopRef.current = chatStop;
  const isTranscriptUnavailable =
    isBatchTranscriptionPending && !hasAvailableTranscript;

  useMountEffect(() => {
    acceptFinishedChatPersistenceRef.current = true;
    chatCloudsyncActivity.resume();
    return () => {
      acceptFinishedChatPersistenceRef.current = false;
      const pendingChatGroupIds = new Set([
        ...submittedChatGroupIdsRef.current.values(),
        ...[...pendingRegenerationTombstonesRef.current.values()].map(
          ({ chatGroupId }) => chatGroupId,
        ),
        ...(latestChatGroupIdRef.current ? [latestChatGroupIdRef.current] : []),
      ]);
      try {
        void Promise.resolve(latestChatStopRef.current()).catch((error) => {
          console.error("Failed to stop chat response during cleanup", error);
        });
      } catch (error) {
        console.error("Failed to stop chat response during cleanup", error);
      }
      const cleanup = Promise.resolve().then(async () => {
        try {
          await Promise.resolve();
          await Promise.allSettled([
            ...pendingFinishedChatPersistsRef.current.values(),
            ...[...pendingChatGroupIds].map(waitForPendingChatPersists),
          ]);
          await flushDatabaseWritesByPrefix(["chat:"]);
        } catch (error) {
          console.error("Failed to flush chat writes during cleanup", error);
        }
      });
      void chatCloudsyncActivity.dispose(cleanup);
      pendingTransportPreflightsRef.current.clear();
      pendingRegenerationTombstonesRef.current.clear();
    };
  });

  useEffect(() => {
    if (
      status !== "ready" ||
      !chatGroupId ||
      // An outbound send's write (with retries) may still be in flight; the
      // persisted set is legitimately behind it and reconciling now would
      // wipe the turn the user just sent.
      hasPendingChatPersist(chatGroupId) ||
      // The SDK becomes ready before onFinish's assistant write settles.
      // Keep the completed in-memory response visible until SQLite catches up.
      pendingFinishedChatPersistsRef.current.size > 0 ||
      areMessagesEqual(messages, persistedVisibleMessages)
    ) {
      return;
    }

    chatSetMessages(persistedVisibleMessages);
  }, [
    chatGroupId,
    messages,
    persistedVisibleMessages,
    status,
    chatSetMessages,
  ]);

  const sendMessage = useCallback(
    (message: AnlgUIMessage, options?: ChatSendOptions) => {
      const targetChatGroupId =
        options?.chatGroupId ?? latestChatGroupIdRef.current;
      if (isTranscriptUnavailable && targetChatGroupId && ownerUserId) {
        const assistantMessage: AnlgUIMessage = {
          id: id(),
          role: "assistant",
          parts: [
            {
              type: "text",
              text: t`This recording is using batch transcription, so the transcript isn't available to chat yet. Ask again after transcription finishes, or switch to a Pro model for live transcription.`,
            },
          ],
          metadata: {
            createdAt: Math.max(
              Date.now() + 1,
              (message.metadata?.createdAt ?? 0) + 1,
            ),
          },
        };
        chatSetMessages((current) => [...current, message, assistantMessage]);
        const localResponse = chatCloudsyncActivity.runWithLease(
          message.id,
          async () => {
            try {
              const trackedCompletions: Promise<unknown>[] = [];
              await options?.beforeSend?.((completion) => {
                trackedCompletions.push(completion);
              });
              await upsertChatMessage(
                buildPersistedChatMessage({
                  message: assistantMessage,
                  chatGroupId: targetChatGroupId,
                  ownerUserId,
                  status: "ready",
                }),
              );
              await Promise.allSettled(trackedCompletions);
            } catch (error) {
              await Promise.allSettled([
                deleteChatMessage(targetChatGroupId, message.id),
                deleteChatMessage(targetChatGroupId, assistantMessage.id),
              ]);
              throw error;
            }
          },
        );
        pendingFinishedChatPersistsRef.current.set(message.id, localResponse);
        void localResponse
          .catch((error) => {
            chatSetMessages((current) =>
              current.filter(
                (candidate) =>
                  candidate.id !== message.id &&
                  candidate.id !== assistantMessage.id,
              ),
            );
            console.error(
              "Failed to save batch transcription chat response",
              error,
            );
          })
          .finally(() => {
            if (
              pendingFinishedChatPersistsRef.current.get(message.id) ===
              localResponse
            ) {
              pendingFinishedChatPersistsRef.current.delete(message.id);
            }
          });
        return;
      }
      if (targetChatGroupId) {
        submittedChatGroupIdsRef.current.set(message.id, targetChatGroupId);
      }
      const preflight = options?.beforeSend;
      if (preflight) {
        const queue =
          pendingTransportPreflightsRef.current.get(message.id) ?? [];
        queue.push({ run: preflight, persistOnCancel: true });
        pendingTransportPreflightsRef.current.set(message.id, queue);
      }
      // AnlgUIMessage is structurally compatible with CreateUIMessage<AnlgUIMessage>:
      // no `text`/`files` so the SDK takes the `else` branch and uses message.id as the message id.
      let sending: Promise<void>;
      try {
        sending = Promise.resolve(
          chatSendMessage(message as Parameters<typeof chatSendMessage>[0]),
        );
      } catch (error) {
        if (preflight) {
          removeTransportPreflight(message.id, preflight);
        }
        submittedChatGroupIdsRef.current.delete(message.id);
        console.error("Failed to send chat message", error);
        return;
      }
      void sending.catch((error) => {
        if (preflight) {
          removeTransportPreflight(message.id, preflight);
        }
        submittedChatGroupIdsRef.current.delete(message.id);
        console.error("Failed to send chat message", error);
      });
    },
    [
      chatCloudsyncActivity,
      chatSendMessage,
      chatSetMessages,
      isTranscriptUnavailable,
      ownerUserId,
      removeTransportPreflight,
      t,
    ],
  );

  const regenerate = useCallback(() => {
    if (
      isTranscriptUnavailable ||
      !chatGroupId ||
      (status !== "ready" && status !== "error") ||
      regenerateRequestInFlightRef.current
    ) {
      return;
    }
    let submittedUserIndex = -1;
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].role === "user") {
        submittedUserIndex = i;
        break;
      }
    }
    const submittedUserMessage = messages[submittedUserIndex];
    if (!submittedUserMessage) {
      return;
    }
    const submittedUser = submittedUserMessage;
    const priorFinishedPersist = pendingFinishedChatPersistsRef.current.get(
      submittedUser.id,
    );

    const runRegenerate = (assistantMessageId?: string) => {
      let regenerationPreflight: (() => Promise<void>) | undefined;
      const nextTarget = assistantMessageId
        ? {
            chatGroupId,
            assistantMessageId,
          }
        : undefined;
      const currentTarget = pendingRegenerationTombstonesRef.current.get(
        submittedUser.id,
      );
      const initializeTarget = () => {
        const liveTarget = pendingRegenerationTombstonesRef.current.get(
          submittedUser.id,
        );
        if (liveTarget?.chatGroupId === chatGroupId) {
          return;
        }
        if (nextTarget) {
          pendingRegenerationTombstonesRef.current.set(
            submittedUser.id,
            nextTarget,
          );
        } else if (liveTarget) {
          pendingRegenerationTombstonesRef.current.delete(submittedUser.id);
        }
      };

      if (priorFinishedPersist) {
        regenerationPreflight = async () => {
          await priorFinishedPersist;
          initializeTarget();
        };
        const queue =
          pendingTransportPreflightsRef.current.get(submittedUser.id) ?? [];
        queue.push({
          run: regenerationPreflight,
          persistOnCancel: false,
        });
        pendingTransportPreflightsRef.current.set(submittedUser.id, queue);
      } else if (!currentTarget || currentTarget.chatGroupId !== chatGroupId) {
        initializeTarget();
      }
      regenerateRequestInFlightRef.current = true;
      let regeneration: Promise<void>;
      try {
        regeneration = Promise.resolve(chatRegenerate());
      } catch (error) {
        regeneration = Promise.reject(error);
      }
      const observedRegeneration = regeneration.catch((error) => {
        console.error("Failed to regenerate chat message", error);
      });
      void observedRegeneration
        .then(() => {
          regenerateRequestInFlightRef.current = false;
          return pendingFinishedChatPersistsRef.current.get(submittedUser.id);
        })
        .finally(() => {
          if (!regenerationPreflight) {
            return;
          }
          const queue = pendingTransportPreflightsRef.current.get(
            submittedUser.id,
          );
          const index =
            queue?.findIndex(
              (candidate) => candidate.run === regenerationPreflight,
            ) ?? -1;
          if (queue && index !== -1) {
            queue.splice(index, 1);
          }
          if (queue?.length === 0) {
            pendingTransportPreflightsRef.current.delete(submittedUser.id);
          }
        })
        .catch(() => undefined);
    };

    for (let i = messages.length - 1; i > submittedUserIndex; i--) {
      if (messages[i].role !== "assistant") {
        continue;
      }

      void runRegenerate(messages[i].id);
      return;
    }
    void runRegenerate();
  }, [chatGroupId, messages, chatRegenerate, isTranscriptUnavailable, status]);

  const stop = useCallback(() => {
    void Promise.resolve(chatStop()).catch((error) => {
      console.error("Failed to stop chat response", error);
    });
  }, [chatStop]);

  const setMessages = useCallback(
    (next: AnlgUIMessage[] | ((prev: AnlgUIMessage[]) => AnlgUIMessage[])) => {
      chatSetMessages(next);
      if (!chatGroupId) return;
      const resolved = typeof next === "function" ? next(messages) : next;
      void deleteChatMessagesExcept(
        chatGroupId,
        resolved.map((message) => message.id),
      ).catch((error) => {
        console.error("Failed to reconcile persisted chat messages", error);
      });
    },
    [chatGroupId, messages, chatSetMessages],
  );

  const prevUserMsgCountRef = useRef(0);
  useEffect(() => {
    const count = messages.filter((message) => message.role === "user").length;
    if (count > prevUserMsgCountRef.current) {
      setPendingManualRefs([]);
      setPendingDraftRefs([]);
    }
    prevUserMsgCountRef.current = count;
  }, [messages]);

  const pendingMessageRefs = useMemo(
    () => dedupeByKey([pendingManualRefs, pendingDraftRefs]),
    [pendingManualRefs, pendingDraftRefs],
  );

  const { contextEntities, pendingRefs } = useChatContextPipeline({
    messages,
    currentSessionId,
    pendingManualRefs: pendingMessageRefs,
  });

  const content = children({
    sessionId,
    messages,
    setMessages,
    sendMessage,
    regenerate,
    stop,
    status,
    error,
    contextEntities,
    pendingRefs,
    onRemoveContextEntity,
    onAddContextEntity,
    onDraftContextRefsChange,
    isSystemPromptReady,
  });

  if (unstyled) {
    return content;
  }

  return <div className="flex min-h-0 flex-1 flex-col">{content}</div>;
}

const unavailableChatTransport: ChatTransport<AnlgUIMessage> = {
  sendMessages: async () => {
    throw new Error("Chat model is not ready");
  },
  reconnectToStream: async () => null,
};
