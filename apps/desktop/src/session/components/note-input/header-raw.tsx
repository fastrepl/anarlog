import { useLingui } from "@lingui/react/macro";
import { TextAlignLeft } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
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
  const session = useSession(sessionId);
  const standaloneLabel = standalone ? session?.title.trim() : undefined;

  if (!isActive) {
    return (
      <HeaderViewRawButton
        isActive={isActive}
        label={standaloneLabel}
        onClick={onClick}
        standalone={standalone}
      />
    );
  }

  return (
    <HeaderViewRawActive
      isActive={isActive}
      label={standaloneLabel}
      onClick={onClick}
      rawMd={session?.raw_md}
      sessionId={sessionId}
      standalone={standalone}
    />
  );
}

function HeaderViewRawButton({
  isActive,
  label,
  onClick,
  onContextMenu,
  standalone,
}: {
  isActive: boolean;
  label?: string;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  standalone: boolean;
}) {
  const { t } = useLingui();

  return (
    <IconHeaderView
      isActive={isActive}
      label={label || t`Memos`}
      icon={<TextAlignLeft {...stylex.props(styles.icon)} />}
      onClick={onClick}
      onContextMenu={onContextMenu}
      size={standalone ? "standalone" : "tray"}
      title={label}
      sx={standalone ? styles.standalone : undefined}
    />
  );
}

const styles = stylex.create({
  icon: {
    height: "1rem",
    width: "1rem",
  },
  standalone: {
    borderWidth: 0,
    boxShadow: "none",
  },
});

function HeaderViewRawActive({
  isActive,
  label,
  onClick,
  rawMd,
  sessionId,
  standalone,
}: {
  isActive: boolean;
  label?: string;
  onClick?: () => void;
  rawMd?: string;
  sessionId: string;
  standalone: boolean;
}) {
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
      label={label}
      onClick={onClick}
      onContextMenu={showContextMenu}
      standalone={standalone}
    />
  );
}
