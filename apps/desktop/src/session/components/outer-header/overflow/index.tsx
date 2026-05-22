import {
  FileTextIcon,
  MoreHorizontalIcon,
  PictureInPicture2Icon,
} from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";

import { DeleteNote, DeleteRecording } from "./delete";
import { ExportModal } from "./export-modal";
import { Listening } from "./listening";
import { Copy, ShowInFinder } from "./misc";

import { useAudioPlayer } from "~/audio-player";
import { openFloatingMeetingPanel } from "~/meeting-float/host";
import { useHasTranscript } from "~/session/components/shared";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";

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
  const { audioExists } = useAudioPlayer();
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const canOpenFloatingPanel =
    sessionMode === "active" || sessionMode === "finalizing";
  const openExportModal = () => {
    setOpen(false);
    requestAnimationFrame(() => setIsExportModalOpen(true));
  };
  const handleOpenFloatingPanel = () => {
    setOpen(false);
    void openFloatingMeetingPanel(sessionId);
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
        <DropdownMenuContent variant="app" align="end" className="w-56">
          <AppFloatingPanel className="overflow-hidden p-1">
            <Copy />
            <DropdownMenuItem
              onClick={openExportModal}
              className="cursor-pointer"
            >
              <FileTextIcon />
              <span>Export</span>
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <Listening sessionId={sessionId} hasTranscript={hasTranscript} />
            {canOpenFloatingPanel && (
              <DropdownMenuItem
                onClick={handleOpenFloatingPanel}
                className="cursor-pointer"
              >
                <PictureInPicture2Icon />
                <span>Open floating panel</span>
              </DropdownMenuItem>
            )}
            <DropdownMenuSeparator />
            <ShowInFinder sessionId={sessionId} />
            {audioExists && <DropdownMenuSeparator />}
            {audioExists && <DeleteRecording sessionId={sessionId} />}
            <DeleteNote sessionId={sessionId} />
          </AppFloatingPanel>
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
