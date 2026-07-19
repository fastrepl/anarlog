import { Button } from "@hypr/ui/components/ui/button";

import { useTabs } from "~/store/zustand/tabs";

export function GoogleCalendarAiBoundaryError({
  checkFailed,
}: {
  checkFailed: boolean;
}) {
  const openNew = useTabs((state) => state.openNew);

  return (
    <div
      role="alert"
      className="flex h-full min-h-[400px] flex-col items-center justify-center px-6"
    >
      <div className="mb-6 flex max-w-md flex-col gap-2 text-center">
        <p className="text-base font-medium">
          {checkFailed
            ? "AI is temporarily unavailable"
            : "Use on-device AI for Google Calendar notes"}
        </p>
        <p className="text-muted-foreground text-sm leading-relaxed">
          {checkFailed
            ? "Anarlog could not verify the local calendar data boundary. Restart the app and try again."
            : "To keep Google Calendar data on your device, Anarlog disables hosted and remote AI while that data is present. Choose Ollama or LM Studio to process notes locally."}
        </p>
      </div>
      {!checkFailed && (
        <Button
          variant="outline"
          className="shadow-none"
          onClick={() =>
            openNew({ type: "settings", state: { tab: "intelligence" } })
          }
        >
          Choose on-device AI
        </Button>
      )}
    </div>
  );
}
