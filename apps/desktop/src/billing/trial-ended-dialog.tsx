import { arch, platform } from "@tauri-apps/plugin-os";
import { useEffect } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";

import { TrialDialogIcon } from "./trial-dialog-icon";

import { trackAnalyticsEvent } from "~/analytics";
import {
  GlassDialogCancelButton,
  GlassDialogContent,
} from "~/shared/ui/glass-dialog";
import { isDesktopLocalSttAvailable } from "~/stt/capabilities";

interface TrialEndedDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onUpgrade: () => void;
}

export function TrialEndedDialog({
  open,
  onOpenChange,
  onUpgrade,
}: TrialEndedDialogProps) {
  const supportsFreeLocalTranscription = isDesktopLocalSttAvailable(
    platform(),
    arch(),
  );
  useEffect(() => {
    if (!open) return;
    trackAnalyticsEvent("paywall_viewed", {
      entry_point: "trial_ended_dialog",
      feature: "pro_plan",
    });
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent>
        <DialogHeader className="items-center gap-2 text-center sm:text-center">
          <TrialDialogIcon state="ended" />
          <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
            Your Pro trial has ended
          </DialogTitle>
          <DialogDescription className="text-foreground w-full text-center text-[13px] leading-[1.36]">
            {supportsFreeLocalTranscription
              ? "Your notes and recordings are safe. Free local transcription still works. Upgrade anytime to keep Pro features."
              : "Your notes and recordings are safe. Upgrade anytime to keep cloud transcription and Pro features, or configure your own transcription provider."}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="grid grid-cols-2 gap-2 sm:grid-cols-2 sm:justify-normal">
          <GlassDialogCancelButton onClick={() => onOpenChange(false)}>
            Maybe later
          </GlassDialogCancelButton>
          <Button
            className="bg-primary text-primary-foreground hover:bg-primary/90 h-8 rounded-full px-4 text-xs font-medium shadow-sm dark:bg-white dark:text-black dark:hover:bg-white/90"
            onClick={() => {
              onUpgrade();
              onOpenChange(false);
            }}
          >
            Upgrade to Pro
          </Button>
        </DialogFooter>
      </GlassDialogContent>
    </Dialog>
  );
}
