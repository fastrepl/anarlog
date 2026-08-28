import { t } from "@lingui/core/macro";
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
import { trialDialogStyles } from "./trial-dialog-styles";

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
        <DialogHeader sx={trialDialogStyles.header}>
          <TrialDialogIcon state="ended" />
          <DialogTitle sx={trialDialogStyles.title}>
            {t`Your Pro trial has ended`}
          </DialogTitle>
          <DialogDescription sx={trialDialogStyles.description}>
            {supportsFreeLocalTranscription
              ? t`Your notes and recordings are safe. Free local transcription still works. Upgrade anytime to keep Pro features.`
              : t`Your notes and recordings are safe. Upgrade anytime to keep cloud transcription and Pro features, or configure your own transcription provider.`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter sx={trialDialogStyles.footer}>
          <GlassDialogCancelButton onClick={() => onOpenChange(false)}>
            {t`Maybe later`}
          </GlassDialogCancelButton>
          <Button
            sx={trialDialogStyles.action}
            onClick={() => {
              onUpgrade();
              onOpenChange(false);
            }}
          >
            {t`Upgrade to Pro`}
          </Button>
        </DialogFooter>
      </GlassDialogContent>
    </Dialog>
  );
}
