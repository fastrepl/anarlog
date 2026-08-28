import { Trans, useLingui } from "@lingui/react/macro";
import {
  CaretDown,
  CaretUp,
  Repeat,
  Swap,
  TextAa,
  Textbox,
  X,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import { useEffect, useRef } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import type { NoteEditorRef } from "@anlg/editor/note";
import { Kbd } from "@anlg/ui/components/ui/kbd";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

import { useSearch } from "./context";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

function ToggleButton({
  active,
  onClick,
  tooltip,
  children,
}: {
  active: boolean;
  onClick: () => void;
  tooltip: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={onClick}
          {...stylex.props(
            styles.iconButton,
            active ? styles.toggleActive : styles.toggleInactive,
          )}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" sx={styles.tooltip}>
        {tooltip}
      </TooltipContent>
    </Tooltip>
  );
}

function IconButton({
  onClick,
  disabled,
  tooltip,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  tooltip: React.ReactNode;
  children: React.ReactNode;
}) {
  const btn = (
    <button
      onClick={onClick}
      disabled={disabled}
      {...stylex.props(
        styles.iconButton,
        disabled ? styles.iconButtonDisabled : styles.iconButtonEnabled,
      )}
    >
      {children}
    </button>
  );

  if (disabled) return btn;

  return (
    <Tooltip>
      <TooltipTrigger asChild>{btn}</TooltipTrigger>
      <TooltipContent side="bottom" sx={styles.tooltip}>
        {tooltip}
      </TooltipContent>
    </Tooltip>
  );
}

export function SearchBar({
  editorRef,
}: {
  editorRef: React.RefObject<NoteEditorRef | null>;
}) {
  const { t } = useLingui();
  const search = useSearch();
  const primaryModifier = platform() === "macos" ? "⌘" : "Ctrl";
  const searchInputRef = useRef<HTMLInputElement>(null);
  const replaceInputRef = useRef<HTMLInputElement>(null);

  useMountEffect(() => {
    searchInputRef.current?.focus();

    const editor = editorRef.current;
    return () => editor?.commands.setSearch("", false);
  });

  useEffect(() => {
    if (search?.showReplace) {
      replaceInputRef.current?.focus();
    }
  }, [search?.showReplace]);

  if (!search) {
    return null;
  }

  const {
    query,
    currentMatchIndex,
    totalMatches,
    onNext,
    onPrev,
    caseSensitive,
    wholeWord,
    showReplace,
    replaceQuery,
    toggleWholeWord,
    toggleReplace,
    setReplaceQuery,
  } = search;

  const commands = editorRef.current?.commands;

  const setQuery = (q: string) => {
    search.setQuery(q);
    commands?.setSearch(q, caseSensitive);
  };

  const toggleCaseSensitive = () => {
    search.toggleCaseSensitive();
    commands?.setSearch(query, !caseSensitive);
  };

  const close = () => {
    search.close();
    commands?.setSearch("", false);
  };

  const replaceCurrent = () => {
    if (!query || totalMatches === 0) return;
    commands?.replace({
      query,
      replacement: replaceQuery,
      caseSensitive,
      wholeWord,
      all: false,
      matchIndex: currentMatchIndex,
    });
  };

  const replaceAll = () => {
    if (!query) return;
    commands?.replace({
      query,
      replacement: replaceQuery,
      caseSensitive,
      wholeWord,
      all: true,
      matchIndex: 0,
    });
  };

  const handleSearchKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) {
        onPrev();
      } else {
        onNext();
      }
    }
  };

  const handleReplaceKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.metaKey || e.ctrlKey) {
        replaceAll();
      } else {
        replaceCurrent();
      }
    }
  };

  const displayCount =
    totalMatches > 0 ? `${currentMatchIndex + 1}/${totalMatches}` : "0/0";

  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.row)}>
        <input
          ref={searchInputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleSearchKeyDown}
          placeholder={t`Search...`}
          {...stylex.props(styles.input)}
        />
        <div {...stylex.props(styles.controls)}>
          <ToggleButton
            active={caseSensitive}
            onClick={toggleCaseSensitive}
            tooltip={t`Match case`}
          >
            <TextAa {...stylex.props(styles.icon)} />
          </ToggleButton>
          <ToggleButton
            active={wholeWord}
            onClick={toggleWholeWord}
            tooltip={t`Match whole word`}
          >
            <Textbox {...stylex.props(styles.icon)} />
          </ToggleButton>
          <ToggleButton
            active={showReplace}
            onClick={toggleReplace}
            tooltip={
              <>
                <span>
                  <Trans>Replace</Trans>
                </span>
                <Kbd sx={styles.kbdPress}>{primaryModifier} H</Kbd>
              </>
            }
          >
            <Swap {...stylex.props(styles.icon)} />
          </ToggleButton>
        </div>
        <span {...stylex.props(styles.count)}>{displayCount}</span>
        <div {...stylex.props(styles.inlineControls)}>
          <IconButton
            onClick={onPrev}
            disabled={totalMatches === 0}
            tooltip={
              <>
                <span>
                  <Trans>Previous match</Trans>
                </span>
                <Kbd sx={styles.kbdPress}>⇧ ↵</Kbd>
              </>
            }
          >
            <CaretUp {...stylex.props(styles.icon)} />
          </IconButton>
          <IconButton
            onClick={onNext}
            disabled={totalMatches === 0}
            tooltip={
              <>
                <span>
                  <Trans>Next match</Trans>
                </span>
                <Kbd sx={styles.kbdPress}>↵</Kbd>
              </>
            }
          >
            <CaretDown {...stylex.props(styles.icon)} />
          </IconButton>
        </div>
        <IconButton
          onClick={close}
          tooltip={
            <>
              <span>
                <Trans>Close</Trans>
              </span>
              <Kbd sx={styles.kbdPress}>Esc</Kbd>
            </>
          }
        >
          <X {...stylex.props(styles.icon)} />
        </IconButton>
      </div>

      {showReplace && (
        <div {...stylex.props(styles.row)}>
          <input
            ref={replaceInputRef}
            type="text"
            value={replaceQuery}
            onChange={(e) => setReplaceQuery(e.target.value)}
            onKeyDown={handleReplaceKeyDown}
            placeholder={t`Replace with...`}
            {...stylex.props(styles.input)}
          />
          <div {...stylex.props(styles.controls)}>
            <IconButton
              onClick={replaceCurrent}
              tooltip={
                <>
                  <span>
                    <Trans>Replace</Trans>
                  </span>
                  <Kbd sx={styles.kbdPress}>↵</Kbd>
                </>
              }
            >
              <Swap {...stylex.props(styles.icon)} />
            </IconButton>
            <IconButton
              onClick={replaceAll}
              tooltip={
                <>
                  <span>
                    <Trans>Replace all</Trans>
                  </span>
                  <Kbd sx={styles.kbdPress}>{primaryModifier} ↵</Kbd>
                </>
              }
            >
              <Repeat {...stylex.props(styles.icon)} />
            </IconButton>
          </div>
        </div>
      )}
    </div>
  );
}

const kbdPress = stylex.keyframes({
  "0%": {
    boxShadow: "none",
    transform: "translateY(2px)",
  },
  "50%": {
    boxShadow: "none",
    transform: "translateY(2px)",
  },
  "100%": {
    boxShadow:
      "0 1px 0 0 var(--kbd-press-shadow-outer), inset 0 1px 0 0 var(--kbd-press-shadow-inset)",
    transform: "translateY(0)",
  },
});

const styles = stylex.create({
  controls: {
    alignItems: "center",
    display: "flex",
    gap: "0.125rem",
  },
  count: {
    color: colors.mutedForeground,
    fontSize: "0.625rem",
    fontVariantNumeric: "tabular-nums",
    whiteSpace: "nowrap",
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  iconButton: {
    borderRadius: radii.sm,
    padding: "0.125rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color",
  },
  iconButtonDisabled: {
    color: `color-mix(in srgb, ${colors.mutedForeground} 70%, transparent)`,
    cursor: "not-allowed",
  },
  iconButtonEnabled: {
    color: colors.mutedForeground,
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
  },
  inlineControls: {
    alignItems: "center",
    display: "flex",
  },
  kbdPress: {
    animationDuration: "0.4s",
    animationFillMode: "forwards",
    animationName: kbdPress,
    animationTimingFunction: "ease-out",
  },
  input: {
    backgroundColor: "transparent",
    color: {
      default: null,
      "::placeholder": colors.mutedForeground,
    },
    flex: "1",
    fontSize: "0.75rem",
    height: "100%",
    minWidth: 0,
    outline: {
      default: "none",
      ":focus": "none",
    },
  },
  root: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
  },
  row: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.lg,
    display: "flex",
    gap: "0.375rem",
    height: "1.75rem",
    paddingLeft: "0.5rem",
    paddingRight: "0.5rem",
  },
  toggleActive: {
    backgroundColor: colors.accent,
    color: colors.mutedForeground,
  },
  toggleInactive: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    color: colors.mutedForeground,
  },
  tooltip: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
});
