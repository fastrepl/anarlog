import { Trans } from "@lingui/react/macro";
import {
  AppWindow,
  ArrowsClockwise,
  CalendarBlank,
  DotsThree,
  FileArrowDown,
  FileText,
  PictureInPicture,
  Waveform,
} from "@phosphor-icons/react";
import { useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

import { FolderPickerSubmenu } from "../../folder-picker";
import { MetadataPanelContent } from "../metadata";
import { DeleteNote } from "./delete";
import { ExportModal } from "./export-modal";
import { Listening } from "./listening";
import { LockNote } from "./lock-note";
import { ShowInFolder } from "./misc";

import { useAudioPlayer } from "~/audio-player";
import { openFloatingMeetingPanel } from "~/meeting-float/host";
import { isFloatingBarSupported } from "~/meeting-float/support";
import { useRegenerateTranscript } from "~/session/components/note-input/transcript/actions";
import {
  useCurrentNoteHasContent,
  useHasTranscript,
} from "~/session/components/shared";
import { openStandaloneNoteWindow } from "~/session/window";
import { useConfigValue } from "~/shared/config";
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
  const [hasOpenedExportModal, setHasOpenedExportModal] = useState(false);
  const hasTranscript = useHasTranscript(sessionId);
  const currentNoteHasContent = useCurrentNoteHasContent(
    sessionId,
    currentView,
  );
  const { audioExists, audioExistsResolved } = useAudioPlayer();
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);
  const regenerateTranscript = useRegenerateTranscript(sessionId);
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const floatingBarEnabled = useConfigValue("floating_bar_enabled");
  const floatingBarSupported = isFloatingBarSupported();
  const isMeetingInProgress =
    sessionMode === "active" || sessionMode === "finalizing";
  const showListeningAction = allowListening;
  const showRetranscribeAction =
    audioExistsResolved && sessionMode === "inactive" && audioExists;
  const showUploadActions =
    audioExistsResolved &&
    !audioExists &&
    !hasTranscript &&
    !currentNoteHasContent &&
    !isMeetingInProgress;
  const canOpenFloatingPanel =
    floatingBarSupported &&
    allowListening &&
    floatingBarEnabled &&
    sessionMode === "active";
  const hasMeetingActions =
    showListeningAction ||
    showRetranscribeAction ||
    showUploadActions ||
    canOpenFloatingPanel;
  const openExportModal = () => {
    setOpen(false);
    setHasOpenedExportModal(true);
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
  const handleRetranscribe = () => {
    setOpen(false);
    void regenerateTranscript();
  };
  const handleOpenFloatingPanel = () => {
    setOpen(false);
    void openFloatingMeetingPanel({
      sessionId,
      enabled: floatingBarEnabled,
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
            aria-label="More"
            className="text-muted-foreground hover:bg-accent hover:text-foreground rounded-full [&_svg]:size-4"
          >
            <DotsThree className="size-4" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent variant="app" align="end" className="w-56">
          <AppFloatingPanel className={appFloatingMenuPanelClassName}>
            <FolderPickerSubmenu
              sessionId={sessionId}
              onClose={() => setOpen(false)}
            />
            <DropdownMenuSub>
              <DropdownMenuSubTrigger className="cursor-pointer">
                <CalendarBlank />
                <span>
                  <Trans>Meeting info</Trans>
                </span>
              </DropdownMenuSubTrigger>
              <DropdownMenuPortal>
                <DropdownMenuSubContent
                  variant="app"
                  className="w-72 overflow-hidden"
                >
                  <MetadataPanelContent sessionId={sessionId} />
                </DropdownMenuSubContent>
              </DropdownMenuPortal>
            </DropdownMenuSub>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              onClick={openExportModal}
              className="cursor-pointer"
            >
              <FileArrowDown />
              <span>
                <Trans>Export</Trans>
              </span>
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            {showListeningAction && (
              <Listening
                sessionId={sessionId}
                resume={audioExists || hasTranscript}
              />
            )}
            {showRetranscribeAction && (
              <DropdownMenuItem
                onClick={handleRetranscribe}
                className="cursor-pointer"
              >
                <ArrowsClockwise />
                <span>Re-transcribe</span>
              </DropdownMenuItem>
            )}
            {showUploadActions && (
              <>
                <DropdownMenuItem
                  onClick={handleUploadAudio}
                  className="cursor-pointer"
                >
                  <Waveform />
                  <span>
                    <Trans>Upload audio</Trans>
                  </span>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={handleUploadTranscript}
                  className="cursor-pointer"
                >
                  <FileText />
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
                <PictureInPicture />
                <span>
                  <Trans>Open floating panel</Trans>
                </span>
              </DropdownMenuItem>
            )}
            {hasMeetingActions && <DropdownMenuSeparator />}
            {!standaloneWindow && (
              <DropdownMenuItem
                onClick={handleOpenStandaloneWindow}
                className="cursor-pointer"
              >
                <AppWindow />
                <span>
                  <Trans>Open in New Window</Trans>
                </span>
              </DropdownMenuItem>
            )}
            <ShowInFolder sessionId={sessionId} />
            <LockNote sessionId={sessionId} />
            <DeleteNote sessionId={sessionId} />
          </AppFloatingPanel>
        </DropdownMenuContent>
      </DropdownMenu>
      {hasOpenedExportModal && (
        <ExportModal
          sessionId={sessionId}
          currentView={currentView}
          open={isExportModalOpen}
          onOpenChange={setIsExportModalOpen}
        />
      )}
    </>
  );
}
