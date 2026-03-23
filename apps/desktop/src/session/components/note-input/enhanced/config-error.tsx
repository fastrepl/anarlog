import { ArrowRightIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";

import type { LLMConnectionStatus } from "~/ai/hooks";
import { useTabs } from "~/store/zustand/tabs";

export function ConfigError({ status }: { status: LLMConnectionStatus }) {
  const openNew = useTabs((state) => state.openNew);

  const handleConfigureClick = () => {
    openNew({ type: "ai", state: { tab: "intelligence" } });
  };

  const message = getMessageForStatus(status);

  return (
    <div className="flex h-full min-h-[400px] flex-col items-center justify-center">
      <p className="mb-6 max-w-lg text-center text-sm text-neutral-700">
        {message}
      </p>
      <Button
        onClick={handleConfigureClick}
        className="flex items-center gap-2"
        variant="default"
      >
        <span>Configure</span>
        <ArrowRightIcon size={16} />
      </Button>
    </div>
  );
}

function getMessageForStatus(status: LLMConnectionStatus): string {
  if (status.status === "pending" && status.reason === "missing_provider") {
    return "You need to configure a language model to summarize this meeting";
  }

  if (status.status === "pending" && status.reason === "missing_model") {
    return "You need to select a model to summarize this meeting";
  }

  if (status.status === "error" && status.reason === "unauthenticated") {
    return "You need to sign in to use Char's language model";
  }

  if (status.status === "error" && status.reason === "missing_config") {
    const missing = status.missing;
    if (missing.includes("api_key") && missing.includes("base_url")) {
      return "You need to configure the API key and base URL for your language model provider";
    }
    if (missing.includes("api_key")) {
      return "You need to configure the API key for your language model provider";
    }
    if (missing.includes("base_url")) {
      return "You need to configure the base URL for your language model provider";
    }
  }

  return "You need to configure a language model to summarize this meeting";
}
