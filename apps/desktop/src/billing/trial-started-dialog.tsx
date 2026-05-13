import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";

import { TrialDialogIcon } from "./trial-dialog-icon";

interface TrialStartedDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  trialDaysRemaining: number | null;
}

export function TrialStartedDialog({
  open,
  onOpenChange,
  trialDaysRemaining,
}: TrialStartedDialogProps) {
  const days = trialDaysRemaining ?? 14;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <TrialDialogIcon state="started" />
          <DialogTitle>Your Pro trial just started</DialogTitle>
          <DialogDescription>
            You have {days} {days === 1 ? "day" : "days"} of full Pro access —
            unlimited cloud AI, longer recordings, and every premium template.
            No payment needed during the trial.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button onClick={() => onOpenChange(false)}>Got it</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
