import { useMutation } from "@tanstack/react-query";
import {
  ArrowUpIcon,
  ChevronRightIcon,
  LoaderCircleIcon,
  LogInIcon,
  SparklesIcon,
} from "lucide-react";
import { useRef, useState } from "react";
import { Streamdown } from "streamdown";
import { useMediaQuery } from "usehooks-ts";
import { Drawer } from "vaul";

import { cn } from "@hypr/utils";

import { sharedPrimaryButtonClassName } from "@/components/shared-note-viewer";
import { useMountEffect } from "@/hooks/useMountEffect";
import {
  SharedNoteChatError,
  type SharedNoteChatMessage,
  streamSharedNoteChat,
} from "@/lib/shared-note-chat";
import type { SharedNoteSnapshot } from "@/lib/shared-notes";

export function SharedNoteChatPanel({
  returnPath,
  signedIn,
  snapshot,
}: {
  returnPath: string;
  signedIn: boolean;
  snapshot: SharedNoteSnapshot;
}) {
  const isDesktop = useMediaQuery("(min-width: 1024px)", {
    defaultValue: false,
    initializeWithValue: false,
  });
  // Render nothing until the media query resolves after hydration, so wide
  // viewports never flash the mobile bottom bar before the desktop aside.
  const [interactive, setInteractive] = useState(false);
  const [desktopOpen, setDesktopOpen] = useState(true);
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
      bottomRef.current?.scrollIntoView({ block: "nearest" }),
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
      return { controller };
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
          streamingRef.current += delta;
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
        setMessages((prev) => [...prev, { role: "assistant", content: reply }]);
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
    const history = [...messages, { role: "user" as const, content }];
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

  if (isDesktop) {
    if (!desktopOpen) {
      return (
        <button
          type="button"
          className={cn([
            "fixed right-4 bottom-4 z-30",
            "surface border-color-subtle inline-flex min-h-11 items-center gap-2 rounded-full border px-5 shadow-sm",
            "text-color hover:bg-surface-subtle font-mono text-sm font-medium transition-colors",
            "focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:outline-hidden",
          ])}
          onClick={() => setDesktopOpen(true)}
        >
          <SparklesIcon className="size-4" aria-hidden="true" />
          Ask anything
        </button>
      );
    }
    return (
      <aside
        aria-label="Ask about this note"
        data-chat-panel-open=""
        className="surface border-color-subtle fixed top-14 right-0 bottom-0 z-30 flex w-[336px] flex-col border-l"
      >
        <header className="border-color-subtle flex items-center justify-between gap-3 border-b px-5 py-4">
          <div className="text-color flex items-center gap-2">
            <SparklesIcon className="size-4" aria-hidden="true" />
            <h2 className="font-mono text-sm font-medium">
              Ask about this note
            </h2>
          </div>
          <button
            type="button"
            className="text-color-muted hover:text-color rounded-full p-2 transition-colors focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:outline-hidden"
            aria-label="Collapse chat panel"
            onClick={() => setDesktopOpen(false)}
          >
            <ChevronRightIcon className="size-4" aria-hidden="true" />
          </button>
        </header>
        {body}
      </aside>
    );
  }

  return (
    <Drawer.Root>
      <div aria-hidden="true" className="h-24" />
      <div className="fixed inset-x-0 bottom-0 z-30 px-4 pb-[max(env(safe-area-inset-bottom),1rem)]">
        <Drawer.Trigger
          className={cn([
            "surface border-color-subtle mx-auto flex min-h-12 w-full max-w-[420px] items-center gap-2 rounded-full border px-5 shadow-lg",
            "text-color-muted font-mono text-sm",
            "focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:outline-hidden",
          ])}
        >
          <SparklesIcon className="size-4" aria-hidden="true" />
          Ask anything
        </Drawer.Trigger>
      </div>
      <Drawer.Portal>
        <Drawer.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <Drawer.Content
          aria-label="Ask about this note"
          className="bg-page border-color-subtle fixed inset-x-0 bottom-0 z-50 flex h-[85dvh] flex-col rounded-t-3xl border-t"
        >
          <div className="bg-surface-subtle mx-auto mt-3 h-1.5 w-10 shrink-0 rounded-full" />
          <Drawer.Title className="text-color flex items-center gap-2 px-5 pt-4 pb-2 font-mono text-sm font-medium">
            <SparklesIcon className="size-4" aria-hidden="true" />
            Ask about this note
          </Drawer.Title>
          <Drawer.Description className="sr-only">
            Chat with AI about this shared note.
          </Drawer.Description>
          {body}
        </Drawer.Content>
      </Drawer.Portal>
    </Drawer.Root>
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
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-5 py-4">
        {messages.length === 0 && streaming === null && (
          <p className="text-color-muted text-sm leading-6">
            Ask anything about this note — a summary, action items, or details
            you may have missed.
          </p>
        )}
        {messages.map((message, index) =>
          message.role === "user" ? (
            <div key={index} className="flex justify-end">
              <p className="surface-subtle text-color max-w-[85%] rounded-2xl px-3.5 py-2 text-sm leading-6 whitespace-pre-wrap">
                {message.content}
              </p>
            </div>
          ) : (
            <div key={index} className="text-color min-w-0 text-sm leading-6">
              <Streamdown>{message.content}</Streamdown>
            </div>
          ),
        )}
        {streaming !== null &&
          (streaming === "" ? (
            <p className="text-color-muted flex items-center gap-2 text-sm">
              <LoaderCircleIcon
                className="size-4 animate-spin"
                aria-hidden="true"
              />
              Thinking…
            </p>
          ) : (
            <div className="text-color min-w-0 text-sm leading-6">
              <Streamdown>{streaming}</Streamdown>
            </div>
          ))}
        {errorMessage && (
          <p className="text-sm text-red-700" role="status">
            {errorMessage}
          </p>
        )}
        <div ref={bottomRef} />
      </div>
      <div className="border-color-subtle border-t px-5 py-4">
        {signedIn ? (
          <form
            className="flex items-end gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              onSend();
            }}
          >
            <textarea
              className="surface-subtle text-color placeholder:text-color-muted min-h-11 flex-1 resize-none rounded-2xl px-4 py-2.5 text-sm leading-6 focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:outline-hidden"
              placeholder="Ask anything"
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
              className={cn([
                "inline-flex size-11 shrink-0 items-center justify-center rounded-full",
                "bg-linear-to-t from-stone-600 to-stone-500 text-white transition-opacity hover:opacity-90",
                "focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:ring-offset-2 focus-visible:outline-hidden",
                "disabled:cursor-not-allowed disabled:opacity-50",
              ])}
              disabled={pending || draft.trim() === ""}
            >
              {pending ? (
                <LoaderCircleIcon
                  className="size-4 animate-spin"
                  aria-hidden="true"
                />
              ) : (
                <ArrowUpIcon className="size-4" aria-hidden="true" />
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
    <div className="surface-subtle border-color-subtle rounded-2xl border px-4 py-5">
      <p className="text-color font-mono text-sm font-medium">
        Sign in to ask about this note
      </p>
      <p className="text-color-muted mt-1 text-sm leading-6">
        Sign in to chat with AI about this shared note.
      </p>
      <a
        href={`/auth/?${search.toString()}`}
        className={cn([sharedPrimaryButtonClassName, "mt-4"])}
      >
        <LogInIcon className="mr-2 size-4" aria-hidden="true" />
        Sign in
      </a>
    </div>
  );
}
