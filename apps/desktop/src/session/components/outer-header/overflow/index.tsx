import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { useState } from "react";

import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import {
  AppWindow,
  ArrowsClockwise,
  CalendarBlank,
  ClockCounterClockwise,
  DotsThree,
  FileArrowDown,
  FileText,
  FolderOpen,
  PictureInPicture,
  Waveform,
} from "@anlg/ui/components/icons";
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
import { toast } from "@anlg/ui/components/ui/toast";

import { MetadataPanelContent } from "../metadata";
import { AlwaysOnTop } from "./always-on-top";
import { DeleteNote } from "./delete";
import { ExportModal } from "./export-modal";
import { Listening } from "./listening";
import { LockNote } from "./lock-note";

import { useAudioPlayer } from "~/audio-player";
import { DEVICE_AUTH_REASON } from "~/lock/auth";
import { isLockedFlag } from "~/lock/flag";
import { useAppLock } from "~/lock/store";
import { openFloatingMeetingPanel } from "~/meeting-float/host";
import { isFloatingBarSupported } from "~/meeting-float/support";
import { useRegenerateTranscript } from "~/session/components/note-input/transcript/actions";
import {
  useCurrentNoteHasContent,
  useHasTranscript,
} from "~/session/components/shared";
import { VersionHistoryDialog } from "~/session/components/version-history-dialog";
import { useSession } from "~/session/queries";
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
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const [isExportModalOpen, setIsExportModalOpen] = useState(false);
  const [hasOpenedExportModal, setHasOpenedExportModal] = useState(false);
  const [exportOpenCount, setExportOpenCount] = useState(0);
  const [isVersionHistoryOpen, setIsVersionHistoryOpen] = useState(false);
  const [hasOpenedVersionHistory, setHasOpenedVersionHistory] = useState(false);
  const hasTranscript = useHasTranscript(sessionId);
  const currentNoteHasContent = useCurrentNoteHasContent(
    sessionId,
    currentView,
  );
  const session = useSession(sessionId);
  const noteLocked = session == null || isLockedFlag(session.locked);
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
  const showAudioInFolder = audioExistsResolved && audioExists;
  const showAudioLabel =
    platform() === "macos"
      ? t`Show Audio in Finder`
      : platform() === "windows"
        ? t`Show Audio in Explorer`
        : t`Show Audio in Folder`;
  const canOpenFloatingPanel =
    floatingBarSupported &&
    allowListening &&
    floatingBarEnabled &&
    sessionMode === "active";
  const hasRecordingActions =
    showListeningAction ||
    showRetranscribeAction ||
    showUploadActions ||
    showAudioInFolder;
  const openExportModal = () => {
    setOpen(false);
    setHasOpenedExportModal(true);
    setExportOpenCount((count) => count + 1);
    requestAnimationFrame(() => setIsExportModalOpen(true));
  };
  const openVersionHistory = () => {
    setOpen(false);
    setHasOpenedVersionHistory(true);
    requestAnimationFrame(() => setIsVersionHistoryOpen(true));
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
  const handleShowAudioInFolder = async () => {
    setOpen(false);
    if (noteLocked) {
      const ok = await useAppLock
        .getState()
        .authenticate(DEVICE_AUTH_REASON.openApp);
      if (!ok) return;
    }
    const exists = await fsSyncCommands.audioExist(sessionId);
    if (exists.status !== "ok" || !exists.data) {
      toast.error(t`Recording not found. It may have been deleted.`, {
        id: `show-audio-missing-${sessionId}`,
      });
      return;
    }
    const dir = await fsSyncCommands.sessionDir(sessionId);
    if (dir.status === "ok") {
      await openerCommands.openPath(dir.data, null);
    }
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
            <DropdownMenuItem
              onClick={openVersionHistory}
              className="cursor-pointer"
            >
              <ClockCounterClockwise />
              <span>
                <Trans>Version history</Trans>
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
            {showAudioInFolder && (
              <DropdownMenuItem
                onClick={handleShowAudioInFolder}
                className="cursor-pointer"
              >
                <FolderOpen />
                <span>{showAudioLabel}</span>
              </DropdownMenuItem>
            )}
            {hasRecordingActions && <DropdownMenuSeparator />}
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
            <AlwaysOnTop />
            <DropdownMenuSeparator />
            <LockNote sessionId={sessionId} />
            <DeleteNote sessionId={sessionId} />
          </AppFloatingPanel>
        </DropdownMenuContent>
      </DropdownMenu>
      {hasOpenedExportModal && (
        <ExportModal
          key={exportOpenCount}
          sessionId={sessionId}
          currentView={currentView}
          open={isExportModalOpen}
          onOpenChange={setIsExportModalOpen}
        />
      )}
      {hasOpenedVersionHistory && (
        <VersionHistoryDialog
          sessionId={sessionId}
          open={isVersionHistoryOpen}
          onOpenChange={setIsVersionHistoryOpen}
        />
      )}
    </>
  );
}
