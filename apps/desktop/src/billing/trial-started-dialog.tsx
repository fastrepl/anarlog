import { t } from "@lingui/core/macro";

import { PRO_TRIAL_DAYS } from "@anlg/pricing";
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

import { GlassDialogContent } from "~/shared/ui/glass-dialog";

interface TrialStartedDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  trialDaysRemaining: number | null;
  hasPaymentMethod: boolean;
}

export function TrialStartedDialog({
  open,
  onOpenChange,
  trialDaysRemaining,
  hasPaymentMethod,
}: TrialStartedDialogProps) {
  const days = trialDaysRemaining ?? PRO_TRIAL_DAYS;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent>
        <DialogHeader sx={trialDialogStyles.header}>
          <TrialDialogIcon state="started" />
          <DialogTitle sx={trialDialogStyles.title}>
            {t`Your Pro trial just started`}
          </DialogTitle>
          <DialogDescription sx={trialDialogStyles.description}>
            {hasPaymentMethod
              ? t`Your ${days}-day Pro trial starts now. Pro will continue automatically when it ends.`
              : t`Your ${days}-day Pro trial starts now. Add a payment method before it ends to keep Pro.`}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter sx={trialDialogStyles.footerCentered}>
          <Button
            sx={[trialDialogStyles.action, trialDialogStyles.wideAction]}
            onClick={() => onOpenChange(false)}
          >
            {t`Got it`}
          </Button>
        </DialogFooter>
      </GlassDialogContent>
    </Dialog>
  );
}
