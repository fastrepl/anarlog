import { ArrowUp, CircleNotch, SignIn, Sparkle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { Streamdown } from "streamdown";

import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogTrigger,
} from "@anlg/ui/components/ui/dialog";
import { cn } from "@anlg/utils";

import { sharedButtonStyles } from "@/components/shared-note-viewer";
import { useMountEffect } from "@/hooks/useMountEffect";
import {
  appendSharedNoteChatMessage,
  appendSharedNoteChatResponse,
  MAX_SHARED_NOTE_CHAT_MESSAGE_CHARS,
  SharedNoteChatError,
  type SharedNoteChatMessage,
  streamSharedNoteChat,
} from "@/lib/shared-note-chat";
import type { SharedNoteSnapshot } from "@/lib/shared-notes";
const styles = stylex.create({
  style1: {
    position: "fixed",
    bottom: "calc(.75rem + env(safe-area-inset-bottom))",
    left: "50%",
    zIndex: "30",
    height: "2.5rem",
    width: "180px",
    maxWidth: "calc(100vw - 2rem)",
    "--tw-translate-x": "calc(calc(1 / 2 * 100%) * -1)",
    translate: "calc(calc(1 / 2 * 100%) * -1) 0",
    cursor: "text",
    "--tw-outline-style": {
      default: null,
      ":focus-visible": "none",
    },
    outlineStyle: {
      default: null,
      ":focus-visible": "none",
    },
  },
  style2: {
    display: "flex",
    alignItems: "center",
    gap: ".75rem",
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    paddingInline: "1.25rem",
    paddingBlock: "1rem",
    paddingRight: "3.5rem",
  },
  style3: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
  },
  style4: {
    width: "1rem",
    height: "1rem",
  },
  style5: {
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, Courier New, monospace",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    "--tw-font-weight": "500",
    fontWeight: "500",
  },
  style6: {
    minHeight: "0",
    flex: "1",
    overflowY: "auto",
    paddingInline: "1.25rem",
    paddingBlock: "1rem",
  },
  style7: {
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
  },
  style8: {
    display: "flex",
    justifyContent: "flex-end",
  },
  style9: {
    maxWidth: "85%",
    borderRadius: "1rem",
    paddingInline: ".875rem",
    paddingBlock: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
    whiteSpace: "pre-wrap",
  },
  style10: {
    minWidth: "0",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
  },
  style11: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
  },
  style12: {
    width: "1rem",
    height: "1rem",
    animation: "1s linear infinite spin",
  },
  style13: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style14: {
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingInline: "1.25rem",
    paddingBlock: "1rem",
  },
  style15: {
    display: "flex",
    alignItems: "flex-end",
    gap: ".5rem",
  },
  style16: {
    minHeight: "2.75rem",
    flex: "1",
    resize: "none",
    borderRadius: "1rem",
    paddingInline: "1rem",
    paddingBlock: ".625rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
    "--tw-ring-shadow": {
      default: null,
      ":focus-visible": " 0 0 0 calc(2px + 0) currentcolor",
    },
    boxShadow: {
      default: null,
      ":focus-visible":
        "0 0 #0000, 0 0 #0000, 0 0 #0000, var(--tw-ring-inset, ) 0 0 0 calc(2px + var(--tw-ring-offset-width)) var(--tw-ring-color, currentcolor), 0 0 #0000",
    },
    "--tw-ring-color": {
      default: null,
      ":focus-visible": "#78716c",
    },
    "--tw-outline-style": {
      default: null,
      ":focus-visible": "none",
    },
    outlineStyle: {
      default: null,
      ":focus-visible": "none",
    },
    outlineOffset: {
      default: null,
      "@media (forced-colors: active)": {
        default: null,
        ":focus-visible": "2px",
      },
    },
    outline: {
      default: null,
      "@media (forced-colors: active)": {
        default: null,
        ":focus-visible": "2px solid #0000",
      },
    },
  },
  style17: {
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: "1.25rem",
  },
  style18: {
    fontFamily:
      "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, Courier New, monospace",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    "--tw-font-weight": "500",
    fontWeight: "500",
  },
  style19: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
  },
  style20: {
    marginRight: ".5rem",
    width: "1rem",
    height: "1rem",
  },
});
export function SharedNoteChatPanel({
  returnPath,
  signedIn,
  snapshot,
}: {
  returnPath: string;
  signedIn: boolean;
  snapshot: SharedNoteSnapshot;
}) {
  const [interactive, setInteractive] = useState(false);
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<SharedNoteChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [streaming, setStreaming] = useState<string | null>(null);
  const streamingRef = useRef("");
  const abortRef = useRef<AbortController | null>(null);
  const bottomRef = useRef<HTMLDivElement | null>(null);
  useMountEffect(() => {
    setInteractive(true);
    return () => abortRef.current?.abort();
  });
  const scrollToBottom = () => {
    requestAnimationFrame(() =>
      bottomRef.current?.scrollIntoView({
        block: "nearest",
      }),
    );
  };
  const sendMutation = useMutation({
    // The controller doubles as the request's identity: every callback of a
    // superseded request bails out so it can never touch the active stream.
    onMutate: () => {
      abortRef.current?.abort();
      const controller = new AbortController();
      abortRef.current = controller;
      streamingRef.current = "";
      setStreaming("");
      return {
        controller,
      };
    },
    mutationFn: async (history: SharedNoteChatMessage[]) => {
      const controller = abortRef.current;
      if (!controller) return;
      await streamSharedNoteChat({
        messages: history,
        snapshot,
        signal: controller.signal,
        onDelta: (delta) => {
          if (abortRef.current !== controller) return;
          streamingRef.current = appendSharedNoteChatResponse(
            streamingRef.current,
            delta,
          );
          setStreaming(streamingRef.current);
          scrollToBottom();
        },
      });
    },
    // A failed or interrupted stream discards its partial reply: keeping it
    // would show an error beside a half answer and feed the fragment into
    // the next request's history.
    onSuccess: (_data, _history, context) => {
      if (abortRef.current !== context.controller) return;
      const reply = streamingRef.current;
      if (reply) {
        setMessages((previous) =>
          appendSharedNoteChatMessage(previous, {
            role: "assistant",
            content: reply,
          }),
        );
      }
    },
    onSettled: (_data, _error, _history, context) => {
      sendInFlightRef.current = false;
      if (abortRef.current !== context?.controller) return;
      streamingRef.current = "";
      setStreaming(null);
      scrollToBottom();
    },
  });

  // isPending only flips after a re-render, so a double submit in the same
  // tick could start two requests and build the second history without the
  // first user turn. The ref blocks re-entry synchronously.
  const sendInFlightRef = useRef(false);
  const send = () => {
    const content = draft.trim();
    if (!content || sendInFlightRef.current) {
      return;
    }
    sendInFlightRef.current = true;
    const history = appendSharedNoteChatMessage(messages, {
      role: "user",
      content,
    });
    setMessages(history);
    setDraft("");
    sendMutation.mutate(history);
    scrollToBottom();
  };
  const errorMessage = sendMutation.isError
    ? sendMutation.error instanceof SharedNoteChatError &&
      sendMutation.error.status === 429
      ? "You’ve reached the free AI limit for now. Try again later."
      : "The AI couldn’t answer right now. Please try again."
    : null;
  const body = (
    <ChatBody
      bottomRef={bottomRef}
      draft={draft}
      errorMessage={errorMessage}
      messages={messages}
      pending={sendMutation.isPending}
      returnPath={returnPath}
      signedIn={signedIn}
      streaming={streaming}
      onDraftChange={setDraft}
      onSend={send}
    />
  );
  if (!interactive) {
    return null;
  }
  return (
    <Dialog modal={false} open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <button
          type="button"
          aria-label="Ask anything about this note"
          {...stylex.props(styles.style1)}
        >
          <span
            aria-hidden="true"
            {...stylex.props([
              "pointer-events-none absolute bottom-0 left-1/2 inline-flex h-2 w-[180px] -translate-x-1/2 items-center overflow-hidden rounded-full border border-transparent",
              "origin-bottom bg-[linear-gradient(180deg,#faf8f6_0%,#e3e1df_100%)] px-0 text-sm shadow-[0_0_0_1px_rgba(0,0,0,0.1),0_4px_12px_rgba(0,0,0,0.16),0_4px_16px_rgba(0,0,0,0.1),inset_0_-1px_0_rgba(0,0,0,0.25),inset_0_1px_0_rgba(255,255,255,0.4)] transition-[width,height,padding,background-color,border-color,box-shadow] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)]",
              "group-hover/anarlog-chat-cta:border-stone-300 group-hover/anarlog-chat-cta:bg-[#f4f4f5] group-focus-visible/anarlog-chat-cta:border-stone-300 group-focus-visible/anarlog-chat-cta:bg-[#f4f4f5]",
              "group-hover/anarlog-chat-cta:h-10 group-hover/anarlog-chat-cta:w-[min(640px,calc(100vw-2rem))] group-hover/anarlog-chat-cta:px-4 group-hover/anarlog-chat-cta:shadow-[0_16px_42px_rgba(0,0,0,0.26)]",
              "group-focus-visible/anarlog-chat-cta:h-10 group-focus-visible/anarlog-chat-cta:w-[min(640px,calc(100vw-2rem))] group-focus-visible/anarlog-chat-cta:px-4 group-focus-visible/anarlog-chat-cta:shadow-[0_16px_42px_rgba(0,0,0,0.26)]",
              "group-focus-visible/anarlog-chat-cta:ring-2 group-focus-visible/anarlog-chat-cta:ring-stone-500 group-focus-visible/anarlog-chat-cta:ring-offset-2",
            ])}
          >
            <span
              {...stylex.props([
                "text-color-muted min-w-0 flex-1 truncate text-left opacity-0",
                "transition-opacity duration-100 ease-out",
                "group-hover/anarlog-chat-cta:opacity-100 group-focus-visible/anarlog-chat-cta:opacity-100",
              ])}
            >
              Ask anything about this note
            </span>
          </span>
        </button>
      </DialogTrigger>
      <DialogContent
        showOverlay={false}
        className={[
          [
            "surface border-color-subtle !top-auto !right-4 !bottom-[calc(1rem+env(safe-area-inset-bottom))] !left-4 !z-50 !mx-auto !flex !translate-x-0 !translate-y-0 flex-col overflow-hidden border shadow-2xl",
            "!h-[min(680px,calc(100dvh-5rem-env(safe-area-inset-bottom)))] !w-auto !max-w-[648px] !gap-0 !rounded-[28px] !p-0",
          ],
        ]}
      >
        <header {...stylex.props(styles.style2)}>
          <div {...stylex.props(styles.style3)}>
            <Sparkle {...stylex.props(styles.style4)} aria-hidden="true" />
            <DialogTitle {...stylex.props(styles.style5)}>
              Ask about this note
            </DialogTitle>
          </div>
        </header>
        {body}
      </DialogContent>
    </Dialog>
  );
}
function ChatBody({
  bottomRef,
  draft,
  errorMessage,
  messages,
  onDraftChange,
  onSend,
  pending,
  returnPath,
  signedIn,
  streaming,
}: {
  bottomRef: React.RefObject<HTMLDivElement | null>;
  draft: string;
  errorMessage: string | null;
  messages: SharedNoteChatMessage[];
  onDraftChange: (draft: string) => void;
  onSend: () => void;
  pending: boolean;
  returnPath: string;
  signedIn: boolean;
  streaming: string | null;
}) {
  return (
    <>
      <div {...stylex.props(styles.style6)}>
        {messages.length === 0 && streaming === null && (
          <p {...stylex.props(styles.style7)}>
            Ask anything about this note — a summary, action items, or details
            you may have missed.
          </p>
        )}
        {messages.map((message, index) =>
          message.role === "user" ? (
            <div key={index} {...stylex.props(styles.style8)}>
              <p {...stylex.props(styles.style9)}>{message.content}</p>
            </div>
          ) : (
            <div key={index} {...stylex.props(styles.style10)}>
              <Streamdown>{message.content}</Streamdown>
            </div>
          ),
        )}
        {streaming !== null &&
          (streaming === "" ? (
            <p {...stylex.props(styles.style11)}>
              <CircleNotch
                {...stylex.props(styles.style12)}
                aria-hidden="true"
              />
              Thinking…
            </p>
          ) : (
            <div {...stylex.props(styles.style10)}>
              <Streamdown>{streaming}</Streamdown>
            </div>
          ))}
        {errorMessage && (
          <p {...stylex.props(styles.style13)} role="status">
            {errorMessage}
          </p>
        )}
        <div ref={bottomRef} />
      </div>
      <div {...stylex.props(styles.style14)}>
        {signedIn ? (
          <form
            {...stylex.props(styles.style15)}
            onSubmit={(event) => {
              event.preventDefault();
              onSend();
            }}
          >
            <textarea
              autoFocus
              {...stylex.props(styles.style16)}
              placeholder="Ask anything"
              maxLength={MAX_SHARED_NOTE_CHAT_MESSAGE_CHARS}
              rows={Math.min(3, draft.split("\n").length)}
              value={draft}
              onChange={(event) => onDraftChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  onSend();
                }
              }}
            />
            <button
              type="submit"
              aria-label="Send message"
              {...stylex.props([
                "inline-flex size-11 shrink-0 items-center justify-center rounded-full",
                "bg-linear-to-t from-stone-600 to-stone-500 text-white transition-opacity hover:opacity-90",
                "focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:ring-offset-2 focus-visible:outline-hidden",
                "disabled:cursor-not-allowed disabled:opacity-50",
              ])}
              disabled={pending || draft.trim() === ""}
            >
              {pending ? (
                <CircleNotch
                  {...stylex.props(styles.style12)}
                  aria-hidden="true"
                />
              ) : (
                <ArrowUp {...stylex.props(styles.style4)} aria-hidden="true" />
              )}
            </button>
          </form>
        ) : (
          <SignInToChat returnPath={returnPath} />
        )}
      </div>
    </>
  );
}
function SignInToChat({ returnPath }: { returnPath: string }) {
  const search = new URLSearchParams({
    flow: "web",
    redirect: returnPath,
  });
  return (
    <div {...stylex.props(styles.style17)}>
      <p {...stylex.props(styles.style18)}>Sign in to ask about this note</p>
      <p {...stylex.props(styles.style19)}>
        Sign in to chat with AI about this shared note.
      </p>
      <a
        href={`/auth/?${search.toString()}`}
        {...stylex.props([
          [sharedButtonStyles.base, sharedButtonStyles.primary],
          "mt-4",
        ])}
      >
        <SignIn {...stylex.props(styles.style20)} aria-hidden="true" />
        Sign in
      </a>
    </div>
  );
}
