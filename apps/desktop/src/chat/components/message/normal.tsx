import { useLingui } from "@lingui/react/macro";
import { useCallback, useEffect, useRef, useState } from "react";
import { Streamdown } from "streamdown";

import {
  ArrowCounterClockwise,
  Brain,
  Check,
  CircleNotch,
  Copy,
} from "@anlg/ui/components/icons";
import { streamdownIcons } from "@anlg/ui/components/streamdown-icons";
import { cn } from "@anlg/utils";

import {
  Disclosure,
  FlatDisclosureGroup,
  MessageBubble,
  MessageContainer,
  useFlatDisclosure,
} from "./shared";
import { MessageTimestamp } from "./timestamp";
import { Tool } from "./tool";
import type { Part } from "./types";

import { hasRenderableContent } from "~/chat/components/shared";
import type { AnlgUIMessage } from "~/chat/types";

function getMessageText(message: AnlgUIMessage): string {
  return message.parts
    .filter(
      (part): part is Extract<Part, { type: "text" }> => part.type === "text",
    )
    .map((part) => part.text)
    .join("\n");
}

export function NormalMessage({
  message,
  handleReload,
}: {
  message: AnlgUIMessage;
  handleReload?: () => void;
}) {
  const { t } = useLingui();
  const isUser = message.role === "user";
  const activityParts = isUser ? [] : message.parts.filter(isActivityPart);
  const activityRunning = activityParts.some((part) =>
    part.type === "reasoning"
      ? part.state === "streaming"
      : "state" in part &&
        (part.state === "input-streaming" || part.state === "input-available"),
  );
  const visibleParts = isUser
    ? message.parts
    : message.parts.filter((part) => !isActivityPart(part));
  const [copied, setCopied] = useState(false);
  const copiedResetTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (copiedResetTimeoutRef.current !== null) {
        window.clearTimeout(copiedResetTimeoutRef.current);
      }
    };
  }, []);

  const handleCopy = useCallback(async () => {
    const text = getMessageText(message);
    try {
      await navigator.clipboard.writeText(text);
      if (copiedResetTimeoutRef.current !== null) {
        window.clearTimeout(copiedResetTimeoutRef.current);
      }
      setCopied(true);
      copiedResetTimeoutRef.current = window.setTimeout(() => {
        setCopied(false);
        copiedResetTimeoutRef.current = null;
      }, 2000);
    } catch {
      // ignore
    }
  }, [message]);

  if (!hasRenderableContent(message)) {
    return null;
  }

  return (
    <MessageContainer align={isUser ? "end" : "start"}>
      <div
        className={cn([
          "group/message flex min-w-0 flex-col",
          isUser ? "max-w-[85%] items-end" : "w-full",
        ])}
      >
        <MessageBubble variant={isUser ? "user" : "assistant"}>
          {activityParts.length > 0 && (
            <Disclosure
              icon={
                activityRunning ? (
                  <CircleNotch className="h-3 w-3 animate-spin" />
                ) : (
                  <Brain className="h-3 w-3" />
                )
              }
              title={t`Activity`}
            >
              <FlatDisclosureGroup>
                {activityParts.map((part, i) => (
                  <Part key={i} part={part as Part} />
                ))}
              </FlatDisclosureGroup>
            </Disclosure>
          )}
          {visibleParts.map((part, i) => (
            <Part key={i} part={part as Part} />
          ))}
        </MessageBubble>
        <div className="mt-1 flex min-h-6 items-center gap-1 opacity-0 transition-opacity group-focus-within/message:opacity-100 group-hover/message:opacity-100">
          {!isUser && (
            <>
              <button
                onClick={handleCopy}
                className={`p-1 transition-colors ${copied ? "text-green-500" : "text-muted-foreground hover:text-foreground"}`}
                aria-label={t`Copy message`}
              >
                {copied ? <Check size={14} /> : <Copy size={14} />}
              </button>
              {handleReload && (
                <button
                  onClick={handleReload}
                  className="text-muted-foreground hover:text-foreground p-1 transition-colors"
                  aria-label={t`Regenerate message`}
                >
                  <ArrowCounterClockwise size={14} />
                </button>
              )}
            </>
          )}
          <MessageTimestamp createdAt={message.metadata?.createdAt} />
        </div>
      </div>
    </MessageContainer>
  );
}

function isActivityPart(part: AnlgUIMessage["parts"][number]) {
  if (part.type === "reasoning") {
    return part.text.trim().length > 0;
  }

  return (
    (part.type.startsWith("tool-") || part.type === "dynamic-tool") &&
    part.type !== "tool-edit_memo" &&
    part.type !== "tool-edit_summary" &&
    part.type !== "tool-update_prompt_template"
  );
}

function Part({ part }: { part: Part }) {
  if (part.type === "reasoning") {
    return <Reasoning part={part} />;
  }
  if (part.type === "text") {
    return <Text part={part} />;
  }
  if (part.type === "step-start") {
    return null;
  }

  return <Tool part={part} />;
}

function Reasoning({ part }: { part: Extract<Part, { type: "reasoning" }> }) {
  const flat = useFlatDisclosure();
  const raw = part.text.trim();

  if (!raw) {
    return null;
  }

  const cleaned = raw
    .replace(/[\n`*#"]/g, " ")
    .replace(/\s+/g, " ")
    .trim();

  const streaming = part.state !== "done";
  const title = streaming ? cleaned.slice(-150) : cleaned;

  if (!title) {
    return null;
  }

  if (flat) {
    return (
      <div className="my-1.5 flex items-start gap-2 first:mt-0 last:mb-0">
        {streaming ? (
          <CircleNotch className="mt-0.5 h-3 w-3 shrink-0 animate-spin" />
        ) : (
          <Brain className="text-muted-foreground mt-0.5 h-3 w-3 shrink-0" />
        )}
        <div className="text-muted-foreground min-w-0 flex-1 text-xs whitespace-pre-wrap">
          {part.text}
        </div>
      </div>
    );
  }

  return (
    <Disclosure
      icon={<Brain className="h-3 w-3" />}
      title={title}
      disabled={streaming}
    >
      <div className="text-muted-foreground text-sm whitespace-pre-wrap">
        {part.text}
      </div>
    </Disclosure>
  );
}

const chatComponents = {
  h1: (props: React.HTMLAttributes<HTMLHeadingElement>) => {
    return (
      <h1 className="mt-3 mb-1 text-base font-semibold first:mt-0">
        {props.children as React.ReactNode}
      </h1>
    );
  },
  h2: (props: React.HTMLAttributes<HTMLHeadingElement>) => {
    return (
      <h2 className="mt-3 mb-1 text-base font-semibold first:mt-0">
        {props.children as React.ReactNode}
      </h2>
    );
  },
  h3: (props: React.HTMLAttributes<HTMLHeadingElement>) => {
    return (
      <h3 className="mt-2 mb-1 text-sm font-semibold first:mt-0">
        {props.children as React.ReactNode}
      </h3>
    );
  },
  ul: (props: React.HTMLAttributes<HTMLUListElement>) => {
    return (
      <ul className="overflow-wrap-anywhere mb-1 list-disc pl-5">
        {props.children as React.ReactNode}
      </ul>
    );
  },
  ol: (props: React.HTMLAttributes<HTMLOListElement>) => {
    return (
      <ol className="overflow-wrap-anywhere mb-1 list-decimal pl-5">
        {props.children as React.ReactNode}
      </ol>
    );
  },
  li: (props: React.HTMLAttributes<HTMLLIElement>) => {
    return (
      <li className="overflow-wrap-anywhere mb-1 min-w-0">
        {props.children as React.ReactNode}
      </li>
    );
  },
  p: (props: React.HTMLAttributes<HTMLParagraphElement>) => {
    return (
      <p className="mb-1.5 last:mb-0">{props.children as React.ReactNode}</p>
    );
  },
} as const;

function Text({ part }: { part: Extract<Part, { type: "text" }> }) {
  const isAnimating = part.state !== "done";

  return (
    <Streamdown
      icons={streamdownIcons}
      components={chatComponents}
      className="overflow-wrap-anywhere min-w-0 px-0.5 py-1"
      caret="block"
      isAnimating={isAnimating}
      linkSafety={{ enabled: false }}
    >
      {part.text}
    </Streamdown>
  );
}
