import { Trans } from "@lingui/react/macro";
import {
  AudioLinesIcon,
  FileDownIcon,
  FileTextIcon,
  MoreHorizontalIcon,
  PictureInPicture2Icon,
  SquareArrowOutUpRightIcon,
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

import { useMeetingFloatMainStore } from "~/meeting-float/hooks";
import { openFloatingMeetingPanel } from "~/meeting-float/host";
import {
  hasStoredNoteContent,
  useHasTranscript,
} from "~/session/components/shared";
import { openStandaloneNoteWindow } from "~/session/window";
import { useConfigValue } from "~/shared/config";
import * as main from "~/store/tinybase/store/main";
import * as settingsStore from "~/store/tinybase/store/settings";
import type { EditorView } from "~/store/zustand/tabs/schema";
import { useListener } from "~/stt/contexts";
import { useUploadFile } from "~/stt/useUploadFile";

export function OverflowButton({
  allowListening = true,
  standaloneWindow = false,
  sessionId,
  currentView,
}: {
  allowListening?: boolean;
  standaloneWindow?: boolean;
  sessionId: string;
  currentView: EditorView;
}) {
  const [open, setOpen] = useState(false);
  const [isExportModalOpen, setIsExportModalOpen] = useState(false);
  const hasTranscript = useHasTranscript(sessionId);
  const currentNoteHasContent = useUploadCurrentViewHasContent(
    sessionId,
    currentView,
    hasTranscript,
  );
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");
  const main = useMeetingFloatMainStore();
  const settings = settingsStore.UI.useStore(settingsStore.STORE_ID);
  const isMeetingInProgress =
    sessionMode === "active" || sessionMode === "finalizing";
  const showUploadActions =
    !hasTranscript && !currentNoteHasContent && !isMeetingInProgress;
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
      main,
      store: settings,
    });
  };
  const handleOpenStandaloneWindow = () => {
    setOpen(false);
    void openStandaloneNoteWindow(sessionId);
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
              <span>
                <Trans>Export</Trans>
              </span>
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            {allowListening && (
              <Listening sessionId={sessionId} hasTranscript={hasTranscript} />
            )}
            {showUploadActions && (
              <>
                <DropdownMenuItem
                  onClick={handleUploadAudio}
                  className="cursor-pointer"
                >
                  <AudioLinesIcon />
                  <span>
                    <Trans>Upload audio</Trans>
                  </span>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={handleUploadTranscript}
                  className="cursor-pointer"
                >
                  <FileTextIcon />
                  <span>
                    <Trans>Upload transcript</Trans>
                  </span>
                </DropdownMenuItem>
              </>
            )}
            {canOpenFloatingPanel && (
              <DropdownMenuItem
                onClick={handleOpenFloatingPanel}
                className="cursor-pointer"
              >
                <PictureInPicture2Icon />
                <span>
                  <Trans>Open floating panel</Trans>
                </span>
              </DropdownMenuItem>
            )}
            <DropdownMenuSeparator />
            {!standaloneWindow && (
              <DropdownMenuItem
                onClick={handleOpenStandaloneWindow}
                className="cursor-pointer"
              >
                <SquareArrowOutUpRightIcon />
                <span>
                  <Trans>Open in New Window</Trans>
                </span>
              </DropdownMenuItem>
            )}
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

function useUploadCurrentViewHasContent(
  sessionId: string,
  currentView: EditorView,
  hasTranscript: boolean,
): boolean {
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const enhancedContent = main.UI.useCell(
    "enhanced_notes",
    currentView.type === "enhanced" ? currentView.id : "",
    "content",
    main.STORE_ID,
  );

  if (currentView.type === "raw") {
    return hasStoredNoteContent(rawMd);
  }

  if (currentView.type === "enhanced") {
    return hasStoredNoteContent(enhancedContent);
  }

  return currentView.type === "transcript" ? hasTranscript : true;
}
