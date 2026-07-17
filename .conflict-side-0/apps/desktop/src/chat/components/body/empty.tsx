import {
  ListChecksIcon,
  MailIcon,
  SearchIcon,
  SparklesIcon,
} from "lucide-react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import type { ContextRef } from "~/chat/context/entities";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import { useTabs } from "~/store/zustand/tabs";

const SUGGESTIONS = [
  {
    label: "List action items.",
    icon: ListChecksIcon,
    prompt: "What are my action items from this meeting?",
  },
  {
    label: "Draft follow-up email.",
    icon: MailIcon,
    prompt: "Draft a follow-up email to the participants",
  },
  {
    label: "Find key decisions.",
    icon: SearchIcon,
    prompt: "What were the key decisions that have been made?",
  },
];

export function ChatBodyEmpty({
  isModelConfigured = true,
  hasContext = false,
  onSendMessage,
}: {
  isModelConfigured?: boolean;
  hasContext?: boolean;
  onSendMessage?: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
    contextRefs?: ContextRef[],
  ) => void;
}) {
  const { isDarkAppearance } = useChatAppearance();
  const openNew = useTabs((state) => state.openNew);

  const handleGoToSettings = useCallback(() => {
    openNew({ type: "settings", state: { tab: "intelligence" } });
  }, [openNew]);

  const handleSuggestionClick = useCallback(
    (prompt: string) => {
      onSendMessage?.(prompt, [{ type: "text", text: prompt }]);
    },
    [onSendMessage],
  );

  if (!isModelConfigured) {
    return (
      <div className="flex justify-start py-2 pb-1">
        <div className="flex w-full flex-col">
          <div className="mb-2 flex items-center gap-2">
            <span
              className={cn([
                "text-sm font-medium",
                isDarkAppearance
                  ? "text-primary-foreground"
                  : "text-foreground",
              ])}
            >
              Anarlog AI
            </span>
            <BetaChip isDarkAppearance={isDarkAppearance} />
          </div>
          <p
            className={cn([
              "mb-2 text-sm",
              isDarkAppearance
                ? "text-primary-foreground/80"
                : "text-muted-foreground",
            ])}
          >
            Hi, I'm Anarlog AI. Set up a language model and I'll be ready to
            help.
          </p>
          <button
            onClick={handleGoToSettings}
            className={cn([
              "border-primary bg-primary text-primary-foreground inline-flex w-fit items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-medium",
              "hover:bg-primary/90 shadow-[0_4px_14px_rgba(87,83,78,0.18)] transition-colors",
            ])}
          >
            <SparklesIcon size={12} />
            Open AI Settings
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex justify-start pb-1">
      <div className="flex w-full flex-col">
        {hasContext && (
          <div className="flex flex-col gap-0.5">
            {SUGGESTIONS.map(({ label, icon: Icon, prompt }) => (
              <button
                key={label}
                onClick={() => handleSuggestionClick(prompt)}
                className={cn([
                  "group grid w-full grid-cols-[1.5rem_minmax(0,1fr)] items-center gap-x-1.5 rounded-lg py-2 pr-3 pl-0 text-left text-sm",
                  isDarkAppearance
                    ? "text-primary-foreground/85 hover:bg-primary-foreground/10"
                    : "text-muted-foreground hover:bg-muted/55",
                  "transition-colors",
                ])}
              >
                <span className="flex size-6 items-center justify-center">
                  <Icon
                    size={16}
                    className={cn([
                      "shrink-0 transition-colors",
                      isDarkAppearance
                        ? "text-primary-foreground/55 group-hover:text-primary-foreground/80"
                        : "text-muted-foreground/75 group-hover:text-foreground",
                    ])}
                  />
                </span>
                <span className="min-w-0 truncate">{label}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function BetaChip({ isDarkAppearance }: { isDarkAppearance: boolean }) {
  return (
    <span
      className={cn([
        "rounded-full border px-1.5 py-0.5 text-[10px] font-medium",
        isDarkAppearance
          ? "border-border bg-accent text-accent-foreground"
          : "border-sky-200 bg-sky-100 text-sky-900",
      ])}
    >
      Beta
    </span>
  );
}
