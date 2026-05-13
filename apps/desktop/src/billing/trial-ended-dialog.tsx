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
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <TrialDialogIcon state="ended" />
          <DialogTitle>Your Pro trial has ended</DialogTitle>
          <DialogDescription>
            You're back on the free plan. Your notes and recordings are safe —
            you can still capture and edit them locally. Upgrade to Pro to keep
            cloud AI, longer recordings, and premium templates.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Maybe later
          </Button>
          <Button
            onClick={() => {
              onUpgrade();
              onOpenChange(false);
            }}
          >
            Upgrade to Pro
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
