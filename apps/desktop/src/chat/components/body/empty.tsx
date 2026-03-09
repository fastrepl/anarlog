import {
  ListChecksIcon,
  MailIcon,
  SearchIcon,
  SparklesIcon,
} from "lucide-react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { useTabs } from "~/store/zustand/tabs";

const SUGGESTIONS = [
  { label: "Actions", icon: ListChecksIcon },
  { label: "Draft of all emails", icon: MailIcon },
  { label: "Find key decisions", icon: SearchIcon },
];

export function ChatBodyEmpty({
  isModelConfigured = true,
  onSendMessage,
}: {
  isModelConfigured?: boolean;
  onSendMessage?: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
  ) => void;
}) {
  const openNew = useTabs((state) => state.openNew);

  const handleGoToSettings = useCallback(() => {
    openNew({ type: "ai", state: { tab: "intelligence" } });
  }, [openNew]);

  const handleSuggestionClick = useCallback(
    (label: string) => {
      onSendMessage?.(label, [{ type: "text", text: label }]);
    },
    [onSendMessage],
  );

  if (!isModelConfigured) {
    return (
      <div className="flex justify-start px-3 py-2 pb-4">
        <div className="flex max-w-[80%] min-w-[240px] flex-col">
          <div className="mb-2 flex items-center gap-2">
            <img src="/assets/dynamic.gif" alt="Char" className="h-5 w-5" />
            <span className="text-sm font-medium text-neutral-800">
              Char AI
            </span>
            <BetaChip />
          </div>
          <p className="mb-2 text-sm text-neutral-700">
            Hey! I need you to configure a language model to start chatting with
            me!
          </p>
          <button
            onClick={handleGoToSettings}
            className="inline-flex w-fit items-center gap-1.5 rounded-full border border-neutral-300 bg-white px-3 py-1.5 text-xs text-neutral-700 transition-colors hover:bg-neutral-100"
          >
            <SparklesIcon size={12} />
            Open AI Settings
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex justify-start px-3 pb-4">
      <div className="flex max-w-[80%] min-w-[240px] flex-col">
        <div className="mb-2 flex items-center gap-1">
          <img src="/assets/dynamic.gif" alt="Char" className="h-5 w-5" />
          <span className="text-sm font-medium text-neutral-800">Char AI</span>
          <BetaChip />
        </div>
        <p className="mb-2 text-sm text-neutral-700">
          Hey! I can help you with a lot of cool stuff :)
        </p>
        <div className="flex flex-wrap gap-1.5">
          {SUGGESTIONS.map(({ label, icon: Icon }) => (
            <button
              key={label}
              onClick={() => handleSuggestionClick(label)}
              className={cn([
                "inline-flex items-center gap-1.5 rounded-full border border-neutral-300 bg-white px-3 py-1.5 text-xs text-neutral-700",
                "transition-colors hover:bg-neutral-100",
              ])}
            >
              <Icon size={12} />
              {label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function BetaChip() {
  return (
    <span className="rounded-full bg-sky-100 px-1.5 py-0.5 text-[10px] font-medium text-sky-900">
      Beta
    </span>
  );
}
