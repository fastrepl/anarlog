import { useLingui } from "@lingui/react/macro";
import { TextAlignLeft } from "@phosphor-icons/react";
import { useMemo } from "react";

import {
  IconHeaderView,
  copyTextToClipboard,
  getStoredNoteMarkdown,
} from "./header-shared";

import { useSession } from "~/session/queries";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";

export function HeaderViewRaw({
  isActive,
  onClick = () => {},
  sessionId,
  standalone = false,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
  standalone?: boolean;
}) {
  if (!isActive) {
    return (
      <HeaderViewRawButton
        isActive={isActive}
        onClick={onClick}
        standalone={standalone}
      />
    );
  }

  return (
    <HeaderViewRawActive
      isActive={isActive}
      onClick={onClick}
      sessionId={sessionId}
      standalone={standalone}
    />
  );
}

function HeaderViewRawButton({
  isActive,
  onClick,
  onContextMenu,
  standalone,
}: {
  isActive: boolean;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  standalone: boolean;
}) {
  const { t } = useLingui();

  return (
    <IconHeaderView
      isActive={isActive}
      label={t`Memos`}
      icon={<TextAlignLeft className="size-4" />}
      onClick={onClick}
      onContextMenu={onContextMenu}
      size={standalone ? "standalone" : "tray"}
      className={standalone ? "border-0 shadow-none" : undefined}
    />
  );
}

function HeaderViewRawActive({
  isActive,
  onClick,
  sessionId,
  standalone,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
  standalone: boolean;
}) {
  const rawMd = useSession(sessionId)?.raw_md;
  const memoMarkdown = useMemo(() => getStoredNoteMarkdown(rawMd), [rawMd]);
  const contextMenu = useMemo<MenuItemDef[]>(
    () => [
      {
        id: `copy-memo-${sessionId}`,
        text: "Copy",
        action: () => {
          void copyTextToClipboard(memoMarkdown, {
            success: "Memo copied to clipboard",
            error: "Failed to copy memo",
          });
        },
        disabled: memoMarkdown.length === 0,
      },
    ],
    [memoMarkdown, sessionId],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <HeaderViewRawButton
      isActive={isActive}
      onClick={onClick}
      onContextMenu={showContextMenu}
      standalone={standalone}
    />
  );
}
