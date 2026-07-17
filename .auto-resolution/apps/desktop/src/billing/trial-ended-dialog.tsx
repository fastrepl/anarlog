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
      <DialogContent className="border-border/45 bg-card/95 w-[calc(100vw-48px)] max-w-[320px] gap-0 overflow-hidden rounded-[26px] p-0 shadow-[0_24px_70px_rgba(0,0,0,0.32)] backdrop-blur-xl sm:rounded-[26px] [&>button:last-child]:hidden">
        <DialogHeader className="items-center gap-2 px-5 pt-7 text-center sm:text-center">
          <TrialDialogIcon state="ended" />
          <DialogTitle className="text-foreground text-[13px] leading-5 font-semibold tracking-normal">
            Your Pro trial has ended
          </DialogTitle>
          <DialogDescription className="text-foreground max-w-[260px] text-center text-[13px] leading-[1.36]">
            Your notes and recordings are safe. Free local transcription still
            works. Upgrade anytime to keep Pro features.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="grid grid-cols-2 gap-2 px-4 pt-4 pb-4 sm:grid-cols-2 sm:justify-normal">
          <Button
            variant="ghost"
            className="bg-accent/80 text-foreground hover:bg-accent hover:text-foreground h-8 rounded-full px-4 text-xs font-medium shadow-none"
            onClick={() => onOpenChange(false)}
          >
            Maybe later
          </Button>
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
      </DialogContent>
    </Dialog>
  );
}
