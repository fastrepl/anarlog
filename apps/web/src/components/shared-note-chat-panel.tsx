import { ArrowUp, CircleNotch, SignIn, Sparkle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { Streamdown } from "streamdown";

import {
  colors,
  fonts,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";
import { markdownComponents } from "@anlg/ui/components/markdown";
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogTrigger,
} from "@anlg/ui/components/ui/dialog";

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

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  style1: {
    position: "fixed",
    bottom: "calc(.75rem + env(safe-area-inset-bottom))",
    left: "50%",
    zIndex: 30,
    height: "2.5rem",
    width: "180px",
    maxWidth: "calc(100vw - 2rem)",
    transform: "translateX(-50%)",
    cursor: "text",
    outline: {
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
    borderColor: colors.border,
    paddingInline: "1.25rem",
    paddingBlock: "1rem",
    paddingRight: "3.5rem",
  },
  style3: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    color: colors.foreground,
  },
  style4: {
    width: "1rem",
    height: "1rem",
  },
  style5: {
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
  },
  style6: {
    minHeight: 0,
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    overflowY: "auto",
    paddingInline: "1.25rem",
    paddingBlock: "1rem",
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  style7: {
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: colors.mutedForeground,
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
    backgroundColor: colors.muted,
    color: colors.foreground,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    whiteSpace: "pre-wrap",
  },
  style10: {
    minWidth: 0,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: colors.foreground,
  },
  style11: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  style12: {
    width: "1rem",
    height: "1rem",
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  style13: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style14: {
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: colors.border,
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
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    resize: "none",
    borderRadius: "1rem",
    backgroundColor: colors.muted,
    color: {
      default: colors.foreground,
      "::placeholder": colors.mutedForeground,
    },
    paddingInline: "1rem",
    paddingBlock: ".625rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.mutedForeground}`,
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
  },
  style17: {
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: colors.border,
    backgroundColor: colors.muted,
    paddingInline: "1rem",
    paddingBlock: "1.25rem",
  },
  style18: {
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
  },
  style19: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: colors.mutedForeground,
  },
  style20: {
    marginRight: ".5rem",
    width: "1rem",
    height: "1rem",
  },
  triggerBar: {
    alignItems: "center",
    backgroundImage: "linear-gradient(180deg, #faf8f6 0%, #e3e1df 100%)",
    borderColor: {
      default: "transparent",
      [stylex.when.ancestor(":focus-visible")]: colors.border,
      [stylex.when.ancestor(":hover")]: colors.border,
    },
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    bottom: 0,
    boxShadow: {
      default:
        "0 0 0 1px rgb(0 0 0 / 0.1), 0 4px 12px rgb(0 0 0 / 0.16), 0 4px 16px rgb(0 0 0 / 0.1), inset 0 -1px 0 rgb(0 0 0 / 0.25), inset 0 1px 0 rgb(255 255 255 / 0.4)",
      [stylex.when.ancestor(":focus-visible")]:
        "0 16px 42px rgb(0 0 0 / 0.26), 0 0 0 2px #78716c, 0 0 0 4px white",
      [stylex.when.ancestor(":hover")]: "0 16px 42px rgb(0 0 0 / 0.26)",
    },
    display: "inline-flex",
    fontSize: ".875rem",
    height: {
      default: ".5rem",
      [stylex.when.ancestor(":focus-visible")]: "2.5rem",
      [stylex.when.ancestor(":hover")]: "2.5rem",
    },
    left: "50%",
    overflow: "hidden",
    paddingInline: {
      default: 0,
      [stylex.when.ancestor(":focus-visible")]: "1rem",
      [stylex.when.ancestor(":hover")]: "1rem",
    },
    pointerEvents: "none",
    position: "absolute",
    transform: "translateX(-50%)",
    transformOrigin: "bottom",
    transitionDuration: "150ms",
    transitionProperty:
      "width, height, padding, background-color, border-color, box-shadow",
    transitionTimingFunction: "cubic-bezier(0.22, 1, 0.36, 1)",
    width: {
      default: "180px",
      [stylex.when.ancestor(":focus-visible")]:
        "min(640px, calc(100vw - 2rem))",
      [stylex.when.ancestor(":hover")]: "min(640px, calc(100vw - 2rem))",
    },
  },
  triggerLabel: {
    color: colors.mutedForeground,
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    minWidth: 0,
    opacity: {
      default: 0,
      [stylex.when.ancestor(":focus-visible")]: 1,
      [stylex.when.ancestor(":hover")]: 1,
    },
    overflow: "hidden",
    textAlign: "left",
    textOverflow: "ellipsis",
    transitionDuration: "100ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "ease-out",
    whiteSpace: "nowrap",
  },
  dialog: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "28px",
    borderStyle: "solid",
    borderWidth: "1px",
    bottom: "calc(1rem + env(safe-area-inset-bottom))",
    boxShadow: shadows.lg,
    display: "flex",
    flexDirection: "column",
    gap: 0,
    height: "min(680px, calc(100dvh - 5rem - env(safe-area-inset-bottom)))",
    left: "1rem",
    marginInline: "auto",
    maxWidth: "648px",
    overflow: "hidden",
    padding: 0,
    right: "1rem",
    top: "auto",
    transform: "none",
    width: "auto",
    zIndex: 50,
  },
  sendButton: {
    alignItems: "center",
    backgroundImage: `linear-gradient(to top, ${colors.primary}, ${colors.mutedForeground})`,
    borderRadius: radii.full,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.mutedForeground}, 0 0 0 4px ${colors.card}`,
    },
    color: colors.primaryForeground,
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "inline-flex",
    flexShrink: 0,
    height: "2.75rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.5,
      ":hover": 0.9,
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    width: "2.75rem",
  },
  signInButton: {
    marginTop: "1rem",
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
          {...stylex.props(styles.style1, stylex.defaultMarker())}
        >
          <span aria-hidden="true" {...stylex.props(styles.triggerBar)}>
            <span {...stylex.props(styles.triggerLabel)}>
              Ask anything about this note
            </span>
          </span>
        </button>
      </DialogTrigger>
      <DialogContent showOverlay={false} sx={styles.dialog}>
        <header {...stylex.props(styles.style2)}>
          <div {...stylex.props(styles.style3)}>
            <Sparkle {...stylex.props(styles.style4)} aria-hidden="true" />
            <DialogTitle sx={styles.style5}>Ask about this note</DialogTitle>
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
              <Streamdown components={markdownComponents} controls={false}>
                {message.content}
              </Streamdown>
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
              <Streamdown components={markdownComponents} controls={false}>
                {streaming}
              </Streamdown>
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
              {...stylex.props(styles.sendButton)}
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
        {...stylex.props(
          sharedButtonStyles.base,
          sharedButtonStyles.primary,
          styles.signInButton,
        )}
      >
        <SignIn {...stylex.props(styles.style20)} aria-hidden="true" />
        Sign in
      </a>
    </div>
  );
}
