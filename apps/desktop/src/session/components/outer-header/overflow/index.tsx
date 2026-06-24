import {
  AudioLinesIcon,
  FileDownIcon,
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

import { DeleteNote } from "./delete";
import { ExportModal } from "./export-modal";
import { Listening } from "./listening";
import { ShowInFinder } from "./misc";

import { openFloatingMeetingPanel } from "~/meeting-float/host";
import {
  useCurrentNoteHasContent,
  useHasTranscript,
} from "~/session/components/shared";
import { useConfigValue } from "~/shared/config";
import * as settingsStore from "~/store/tinybase/store/settings";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { useUploadFile } from "~/stt/useUploadFile";

export function OverflowButton({
  allowListening = true,
  sessionId,
  currentView,
}: {
  allowListening?: boolean;
  sessionId: string;
  currentView: EditorView;
}) {
  const [open, setOpen] = useState(false);
  const [isExportModalOpen, setIsExportModalOpen] = useState(false);
  const hasTranscript = useHasTranscript(sessionId);
  const currentNoteHasContent = useCurrentNoteHasContent(
    sessionId,
    currentView,
  );
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");
  const settings = settingsStore.UI.useStore(settingsStore.STORE_ID);
  const canOpenFloatingPanel =
    allowListening && floatingBarEnabled && sessionMode === "active";
  const openExportModal = () => {
    setOpen(false);
    requestAnimationFrame(() => setIsExportModalOpen(true));
  };
  const handleUploadAudio = () => {
    setOpen(false);
    uploadAudio();
  };
  const handleUploadTranscript = () => {
    setOpen(false);
    uploadTranscript();
  };
  const handleOpenFloatingPanel = () => {
    setOpen(false);
    void openFloatingMeetingPanel({
      sessionId,
      enabled: floatingBarEnabled,
      store: settings,
    });
  };

  return (
    <>
      <DropdownMenu open={open} onOpenChange={setOpen}>
        <DropdownMenuTrigger asChild>
          <Button
            size="icon"
            variant="ghost"
            data-tauri-drag-region="false"
            className="text-muted-foreground hover:bg-accent hover:text-foreground rounded-full"
          >
            <MoreHorizontalIcon size={16} />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent variant="app" align="end" className="w-56">
          <AppFloatingPanel className="overflow-hidden p-1">
            <DropdownMenuItem
              onClick={openExportModal}
              className="cursor-pointer"
            >
              <FileDownIcon />
              <span>Export</span>
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            {allowListening && (
              <Listening sessionId={sessionId} hasTranscript={hasTranscript} />
            )}
            {!currentNoteHasContent && (
              <>
                <DropdownMenuItem
                  onClick={handleUploadAudio}
                  className="cursor-pointer"
                >
                  <AudioLinesIcon />
                  <span>Upload audio</span>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={handleUploadTranscript}
                  className="cursor-pointer"
                >
                  <FileTextIcon />
                  <span>Upload transcript</span>
                </DropdownMenuItem>
              </>
            )}
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
