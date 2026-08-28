import {
  autoUpdate,
  computePosition,
  flip,
  offset,
  shift,
} from "@floating-ui/dom";
import { ChatCenteredDots } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";

const styles = stylex.create({
  style1: {
    width: "1rem",
    height: "1rem",
  },
  commentButton: {
    alignItems: "center",
    backgroundColor: {
      default: colors.card,
      ":hover": colors.muted,
    },
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: {
      default: `0 4px 6px -1px color-mix(in srgb, ${colors.foreground} 10%, transparent), 0 2px 4px -2px color-mix(in srgb, ${colors.foreground} 10%, transparent)`,
      ":focus-visible": `0 0 0 2px ${colors.card}, 0 0 0 4px ${colors.mutedForeground}, 0 4px 6px -1px color-mix(in srgb, ${colors.foreground} 10%, transparent), 0 2px 4px -2px color-mix(in srgb, ${colors.foreground} 10%, transparent)`,
    },
    color: colors.foreground,
    display: "inline-flex",
    fontFamily: fonts.mono,
    fontSize: ".75rem",
    fontWeight: 500,
    gap: ".375rem",
    left: 0,
    lineHeight: "1rem",
    outline: {
      default: null,
      ":focus-visible": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    paddingBlock: ".375rem",
    paddingInline: ".75rem",
    position: "fixed",
    top: 0,
    transitionDuration: "150ms",
    transitionProperty: "background-color, border-color, color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    visibility: "hidden",
    zIndex: 50,
  },
});
export type SelectionRect = {
  left: number;
  top: number;
  right: number;
  bottom: number;
};
export function SharedNoteSelectionComment({
  onStart,
  rect,
  visible,
}: {
  rect: SelectionRect | null;
  visible: boolean;
  onStart: () => void;
}) {
  const [pill, setPill] = useState<HTMLButtonElement | null>(null);
  const rectRef = useRef(rect);
  rectRef.current = rect;
  const active = visible && rect !== null;

  // External sync: floating-ui keeps the portal pill aligned with the
  // viewport rect of the current selection.
  useEffect(() => {
    if (!pill || !active) return;
    const reference = {
      getBoundingClientRect: () => {
        const current = rectRef.current ?? {
          bottom: 0,
          left: 0,
          right: 0,
          top: 0,
        };
        return {
          ...current,
          x: current.left,
          y: current.top,
          width: current.right - current.left,
          height: current.bottom - current.top,
        };
      },
    };
    const update = () => {
      void computePosition(reference, pill, {
        middleware: [
          offset(8),
          flip(),
          shift({
            padding: 8,
          }),
        ],
        placement: "top",
        strategy: "fixed",
      }).then(({ x, y }) => {
        pill.style.left = `${x}px`;
        pill.style.top = `${y}px`;
        pill.style.visibility = "visible";
      });
    };
    return autoUpdate(reference, pill, update);
  }, [pill, active, rect]);
  if (!active) return null;
  return createPortal(
    <button
      ref={setPill}
      type="button"
      {...stylex.props(styles.commentButton)}
      onMouseDown={(event) => event.preventDefault()}
      onClick={onStart}
    >
      <ChatCenteredDots {...stylex.props(styles.style1)} aria-hidden="true" />
      Comment
    </button>,
    document.body,
  );
}
