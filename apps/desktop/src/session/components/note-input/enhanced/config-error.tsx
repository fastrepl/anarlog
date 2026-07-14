import { Trans } from "@lingui/react/macro";
import { CircleAlertIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";

import { useTabs } from "~/store/zustand/tabs";

export function ConfigError() {
  const openNew = useTabs((state) => state.openNew);

  return (
    <div
      role="alert"
      className="flex h-full min-h-[400px] flex-col items-center justify-center px-6"
    >
      <CircleAlertIcon
        aria-hidden
        className="text-muted-foreground mb-5 size-9 stroke-[1.5]"
      />
      <div className="mb-6 flex max-w-md flex-col gap-2 text-center">
        <p className="text-base font-medium">
          <Trans>Set up AI summaries</Trans>
        </p>
        <p className="text-muted-foreground text-sm leading-relaxed">
          <Trans>
            Start a Pro trial or add your own LLM API key to generate a summary
            from this transcript.
          </Trans>
        </p>
      </div>
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          onClick={() =>
            openNew({ type: "settings", state: { tab: "account" } })
          }
        >
          <Trans>Get Pro</Trans>
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            openNew({ type: "settings", state: { tab: "intelligence" } })
          }
        >
          <Trans>Add API key</Trans>
        </Button>
      </div>
    </div>
  );
}
