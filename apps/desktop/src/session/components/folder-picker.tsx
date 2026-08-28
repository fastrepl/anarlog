import { useLingui } from "@lingui/react/macro";
import { Check, FolderSimple, Plus } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@anlg/ui/components/ui/command";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";

import { folderDisplayName, normalizeFolderPath } from "~/session/folders";
import {
  useFolderPaths,
  useSession,
  useUpdateSession,
} from "~/session/queries";

const filterFolders = (value: string, search: string) => {
  const haystack = value.toLocaleLowerCase();
  const needle = search.toLocaleLowerCase();
  return haystack.includes(needle) ? 1 : 0;
};

export function FolderPicker({
  sessionId,
  align = "start",
}: {
  sessionId: string;
  align?: "start" | "end";
}) {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const folderId = useSession(sessionId)?.folder_id ?? "";
  const folderPaths = useFolderPaths();
  const updateSession = useUpdateSession(sessionId);
  const currentPath = folderDisplayName(folderId);
  const folders = useMemo(() => {
    if (currentPath && !folderPaths.includes(currentPath)) {
      return collectWithCurrent(folderPaths, currentPath);
    }
    return folderPaths;
  }, [currentPath, folderPaths]);
  const trimmedQuery = query.trim();
  const normalizedQuery = normalizeFolderPath(trimmedQuery);
  const canCreateFolder =
    Boolean(normalizedQuery) && !folders.includes(normalizedQuery ?? "");
  const folderName = normalizedQuery ?? "";

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) {
      setQuery("");
    }
  }, []);

  const handleSelect = useCallback(
    (nextFolderId: string) => {
      const normalized = normalizeFolderPath(nextFolderId);
      if (normalized === null) {
        return;
      }

      setOpen(false);
      setQuery("");
      if (normalized === folderId) {
        return;
      }

      void updateSession({ folder_id: normalized }).catch((error) => {
        console.error("[folder-picker] failed to update folder", error);
      });
    },
    [folderId, updateSession],
  );

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>
        <button
          type="button"
          data-tauri-drag-region="false"
          role="combobox"
          aria-expanded={open}
          aria-label={
            currentPath ? t`Folder: ${currentPath}` : t`Select folder`
          }
          title={currentPath ? currentPath : t`Select folder`}
          {...stylex.props(
            styles.trigger,
            currentPath ? styles.triggerWithFolder : styles.triggerEmpty,
            open && styles.triggerOpen,
          )}
        >
          <FolderSimple {...stylex.props(styles.icon)} aria-hidden="true" />
          {currentPath ? (
            <span {...stylex.props(styles.folderLabel)}>{currentPath}</span>
          ) : null}
        </button>
      </PopoverTrigger>
      <PopoverContent variant="app" align={align} sx={styles.popover}>
        <AppFloatingPanel sx={styles.panel}>
          <Command filter={filterFolders} sx={styles.command}>
            <div {...stylex.props(styles.content)}>
              <CommandInput
                placeholder={t`Search or create folder`}
                value={query}
                onValueChange={setQuery}
                sx={styles.input}
              />
              <div {...stylex.props(styles.divider)} />
              <CommandList>
                <CommandEmpty sx={styles.empty}>
                  {trimmedQuery
                    ? normalizedQuery === null
                      ? t`Enter a valid folder name.`
                      : t`No folders found.`
                    : t`No folders yet.`}
                </CommandEmpty>
                {currentPath ? (
                  <CommandGroup>
                    <CommandItem
                      value={`no-folder ${t`No folder`}`}
                      onSelect={() => handleSelect("")}
                      sx={styles.pointer}
                    >
                      <span {...stylex.props(styles.itemLabel)}>
                        {t`No folder`}
                      </span>
                    </CommandItem>
                  </CommandGroup>
                ) : null}
                {currentPath && (folders.length > 0 || canCreateFolder) ? (
                  <CommandSeparator />
                ) : null}
                {folders.length > 0 ? (
                  <CommandGroup>
                    {folders.map((path) => (
                      <CommandItem
                        key={path}
                        value={path}
                        onSelect={() => handleSelect(path)}
                        sx={styles.pointer}
                      >
                        <FolderSimple
                          {...stylex.props(styles.icon, styles.mutedIcon)}
                        />
                        <span {...stylex.props(styles.itemLabel)}>{path}</span>
                        {path === currentPath ? (
                          <Check {...stylex.props(styles.icon)} />
                        ) : null}
                      </CommandItem>
                    ))}
                  </CommandGroup>
                ) : null}
                {canCreateFolder && normalizedQuery ? (
                  <CommandGroup>
                    <CommandItem
                      value={`create-folder ${normalizedQuery}`}
                      onSelect={() => handleSelect(normalizedQuery)}
                      sx={styles.pointer}
                    >
                      <Plus {...stylex.props(styles.icon)} />
                      <span {...stylex.props(styles.itemLabel)}>
                        {t`Create "${folderName}"`}
                      </span>
                    </CommandItem>
                  </CommandGroup>
                ) : null}
              </CommandList>
            </div>
          </Command>
        </AppFloatingPanel>
      </PopoverContent>
    </Popover>
  );
}

function collectWithCurrent(folderPaths: string[], currentPath: string) {
  return [...folderPaths, currentPath].sort((left, right) =>
    left.localeCompare(right),
  );
}

const styles = stylex.create({
  command: {
    backgroundColor: "transparent",
    borderRadius: "inherit",
    borderWidth: 0,
    height: {
      default: null,
      ":is(*) [cmdk-input-wrapper]": "1.75rem",
    },
    paddingInline: {
      default: null,
      ":is(*) [cmdk-input-wrapper]": 0,
    },
    borderColor: {
      default: null,
      ":is(*) [cmdk-input-wrapper]": "transparent",
    },
  },
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    padding: "1rem",
  },
  divider: {
    backgroundColor: colors.accent,
    height: "1px",
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    paddingBlock: 0,
    textAlign: "left",
  },
  folderLabel: {
    color: {
      default: "rgb(82 82 82)",
      ":is(.dark *)": "rgb(212 212 212)",
    },
    fontSize: "0.75rem",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  icon: {
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  input: {
    height: "1.75rem",
    paddingBlock: 0,
  },
  itemLabel: {
    flex: "1",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  mutedIcon: {
    opacity: 0.7,
  },
  panel: {
    overflow: "hidden",
  },
  pointer: {
    cursor: "pointer",
  },
  popover: {
    overflow: "hidden",
    width: "21.25rem",
  },
  trigger: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: radii.full,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "flex",
    height: "1.75rem",
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.ring}`,
    },
  },
  triggerEmpty: {
    justifyContent: "center",
    width: "1.75rem",
  },
  triggerOpen: {
    backgroundColor: colors.accent,
    color: colors.foreground,
  },
  triggerWithFolder: {
    gap: "0.25rem",
    maxWidth: "100%",
    minWidth: 0,
    paddingInline: "0.375rem",
  },
});

export { styles as folderPickerStyles };
