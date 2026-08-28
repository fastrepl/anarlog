import { useLingui } from "@lingui/react/macro";
import {
  ArrowCounterClockwise,
  Brain,
  Check,
  Copy,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useEffect, useRef, useState } from "react";
import { Streamdown } from "streamdown";

import { colors } from "@anlg/design-system/tokens.stylex";
import { markdownComponents } from "@anlg/ui/components/markdown";

import { Disclosure, MessageBubble, MessageContainer } from "./shared";
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
        data-chat-assistant-message={!isUser || undefined}
        {...stylex.props([
          styles.message,
          isUser ? styles.userMessage : styles.assistantMessage,
        ])}
      >
        <MessageBubble variant={isUser ? "user" : "assistant"}>
          {message.parts.map((part, i) => (
            <Part key={i} part={part as Part} />
          ))}
        </MessageBubble>
        {!isUser && (
          <div {...stylex.props(styles.actions)}>
            <button
              onClick={handleCopy}
              {...stylex.props([
                styles.messageAction,
                copied ? styles.copiedAction : styles.defaultAction,
              ])}
              aria-label={t`Copy message`}
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
            </button>
            {handleReload && (
              <button
                onClick={handleReload}
                {...stylex.props([styles.messageAction, styles.defaultAction])}
                aria-label={t`Regenerate message`}
              >
                <ArrowCounterClockwise size={14} />
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

  return (
    <Disclosure
      icon={<Brain {...stylex.props(styles.smallIcon)} />}
      title={title}
      disabled={streaming}
    >
      <div {...stylex.props(styles.reasoning)}>{part.text}</div>
    </Disclosure>
  );
}

const chatComponents = {
  ...markdownComponents,
  h1: (props: React.HTMLAttributes<HTMLHeadingElement>) => {
    return (
      <h1 {...stylex.props(styles.headingOne)}>
        {props.children as React.ReactNode}
      </h1>
    );
  },
  h2: (props: React.HTMLAttributes<HTMLHeadingElement>) => {
    return (
      <h2 {...stylex.props(styles.headingOne)}>
        {props.children as React.ReactNode}
      </h2>
    );
  },
  h3: (props: React.HTMLAttributes<HTMLHeadingElement>) => {
    return (
      <h3 {...stylex.props(styles.headingThree)}>
        {props.children as React.ReactNode}
      </h3>
    );
  },
  ul: (props: React.HTMLAttributes<HTMLUListElement>) => {
    return (
      <ul {...stylex.props([styles.list, styles.unorderedList])}>
        {props.children as React.ReactNode}
      </ul>
    );
  },
  ol: (props: React.HTMLAttributes<HTMLOListElement>) => {
    return (
      <ol {...stylex.props([styles.list, styles.orderedList])}>
        {props.children as React.ReactNode}
      </ol>
    );
  },
  li: (props: React.HTMLAttributes<HTMLLIElement>) => {
    return (
      <li {...stylex.props(styles.listItem)}>
        {props.children as React.ReactNode}
      </li>
    );
  },
  p: (props: React.HTMLAttributes<HTMLParagraphElement>) => {
    return (
      <p {...stylex.props(styles.paragraph)}>
        {props.children as React.ReactNode}
      </p>
    );
  },
} as const;

function Text({ part }: { part: Extract<Part, { type: "text" }> }) {
  const isAnimating = part.state !== "done";

  return (
    <Streamdown
      components={chatComponents}
      {...stylex.props(styles.streamdown)}
      controls={false}
      isAnimating={isAnimating}
      linkSafety={{ enabled: false }}
    >
      {part.text}
    </Streamdown>
  );
}

const styles = stylex.create({
  message: {
    display: "flex",
    flexDirection: "column",
    minWidth: 0,
  },
  userMessage: {
    alignItems: "flex-end",
    maxWidth: "85%",
  },
  assistantMessage: {
    width: "100%",
  },
  actions: {
    alignItems: "center",
    display: "flex",
    gap: "0.25rem",
    marginTop: "0.25rem",
    opacity: {
      default: 0,
      ":is([data-chat-assistant-message]:hover *)": 1,
    },
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  messageAction: {
    padding: "0.25rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  copiedAction: {
    color: "oklch(72.3% 0.219 149.579)",
  },
  defaultAction: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
  },
  smallIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  reasoning: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    whiteSpace: "pre-wrap",
  },
  headingOne: {
    fontSize: "1rem",
    fontWeight: 600,
    lineHeight: "1.5rem",
    marginBottom: "0.25rem",
    marginTop: {
      default: "0.75rem",
      ":first-child": 0,
    },
  },
  headingThree: {
    fontSize: "0.875rem",
    fontWeight: 600,
    lineHeight: "1.25rem",
    marginBottom: "0.25rem",
    marginTop: {
      default: "0.5rem",
      ":first-child": 0,
    },
  },
  list: {
    marginBottom: "0.25rem",
    paddingLeft: "1.25rem",
  },
  unorderedList: {
    listStyleType: "disc",
  },
  orderedList: {
    listStyleType: "decimal",
  },
  listItem: {
    marginBottom: "0.25rem",
  },
  paragraph: {
    marginBottom: {
      default: "0.375rem",
      ":last-child": 0,
    },
  },
  streamdown: {
    paddingBlock: "0.25rem",
    paddingInline: "0.125rem",
  },
});
