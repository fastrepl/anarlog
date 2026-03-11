import { FileTextIcon, MoreHorizontalIcon } from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";

import { DeleteNote } from "./delete";
import { ExportModal } from "./export-modal";
import { Listening } from "./listening";
import { Copy, Folder, ShowInFinder } from "./misc";

import { useHasTranscript } from "~/session/components/shared";
import type { EditorView } from "~/store/zustand/tabs/schema";

export function OverflowButton({
  sessionId,
  currentView,
}: {
  sessionId: string;
  currentView: EditorView;
}) {
  const [open, setOpen] = useState(false);
  const [isExportModalOpen, setIsExportModalOpen] = useState(false);
  const hasTranscript = useHasTranscript(sessionId);
  const openExportModal = () => {
    setOpen(false);
    requestAnimationFrame(() => setIsExportModalOpen(true));
  };

  return (
    <>
      <DropdownMenu open={open} onOpenChange={setOpen}>
        <DropdownMenuTrigger asChild>
          <Button
            size="icon"
            variant="ghost"
            className="text-neutral-600 hover:text-black"
          >
            <MoreHorizontalIcon size={16} />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-56">
          <Copy />
          <Folder sessionId={sessionId} setOpen={setOpen} />
          <DropdownMenuItem
            onClick={openExportModal}
            className="cursor-pointer"
          >
            <FileTextIcon />
            <span>Export</span>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <Listening sessionId={sessionId} hasTranscript={hasTranscript} />
          <DropdownMenuSeparator />
          <ShowInFinder sessionId={sessionId} />
          <DropdownMenuSeparator />
          <DeleteNote sessionId={sessionId} />
        </DropdownMenuContent>
      </DropdownMenu>
      <ExportModal
        sessionId={sessionId}
        currentView={currentView}
        open={isExportModalOpen}
        onOpenChange={setIsExportModalOpen}
      />
    </>
  );
}
