import { BrainIcon, CheckIcon, CopyIcon, RotateCcwIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Streamdown } from "streamdown";

import { Disclosure, MessageBubble, MessageContainer } from "./shared";
import { Tool } from "./tool";
import type { Part } from "./types";

import { hasRenderableContent } from "~/chat/components/shared";
import type { HyprUIMessage } from "~/chat/types";

function getMessageText(message: HyprUIMessage): string {
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
  message: HyprUIMessage;
  handleReload?: () => void;
}) {
  const isUser = message.role === "user";
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
      <div className="group flex max-w-[80%] flex-col">
        <MessageBubble variant={isUser ? "user" : "assistant"}>
          {message.parts.map((part, i) => (
            <Part key={i} part={part as Part} />
          ))}
        </MessageBubble>
        {!isUser && (
          <div className="mt-1 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
            <button
              onClick={handleCopy}
              className={`p-1 transition-colors ${copied ? "text-green-500" : "text-neutral-400 hover:text-neutral-600"}`}
              aria-label="Copy message"
            >
              {copied ? <CheckIcon size={14} /> : <CopyIcon size={14} />}
            </button>
            {handleReload && (
              <button
                onClick={handleReload}
                className="p-1 text-neutral-400 transition-colors hover:text-neutral-600"
                aria-label="Regenerate message"
              >
                <RotateCcwIcon size={14} />
              </button>
            )}
          </div>
        )}
      </div>
    </MessageContainer>
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
  const cleaned = part.text
    .replace(/[\n`*#"]/g, " ")
    .replace(/\s+/g, " ")
    .trim();

  const streaming = part.state !== "done";
  const title = streaming ? cleaned.slice(-150) : cleaned;

  return (
    <Disclosure
      icon={<BrainIcon className="h-3 w-3" />}
      title={title}
      disabled={streaming}
    >
      <div className="text-sm whitespace-pre-wrap text-neutral-500">
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
      <ul className="mb-1 list-disc pl-5">
        {props.children as React.ReactNode}
      </ul>
    );
  },
  ol: (props: React.HTMLAttributes<HTMLOListElement>) => {
    return (
      <ol className="mb-1 list-decimal pl-5">
        {props.children as React.ReactNode}
      </ol>
    );
  },
  li: (props: React.HTMLAttributes<HTMLLIElement>) => {
    return <li className="mb-1">{props.children as React.ReactNode}</li>;
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
      components={chatComponents}
      className="px-0.5 py-1"
      caret="block"
      isAnimating={isAnimating}
      linkSafety={{ enabled: false }}
    >
      {part.text}
    </Streamdown>
  );
}
