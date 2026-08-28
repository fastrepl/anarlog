import * as stylex from "@stylexjs/stylex";
import {
  forwardRef,
  type ComponentProps,
  type ComponentRef,
  type ForwardedRef,
} from "react";

import { colors, media } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import { DialogContent } from "@anlg/ui/components/ui/dialog";

export const GlassDialogContent = forwardRef<
  ComponentRef<typeof DialogContent>,
  ComponentProps<typeof DialogContent>
>(function GlassDialogContent(
  { className, overlayClassName, overlaySx, sx, ...props },
  forwardedRef,
) {
  return (
    <DialogContent
      {...props}
      ref={(node) => {
        setForwardedRef(forwardedRef, node);
        const closeButton = node?.querySelector<HTMLElement>(
          ":scope > button:last-child",
        );
        if (closeButton) closeButton.hidden = true;
      }}
      className={className}
      overlayClassName={overlayClassName}
      overlaySx={[styles.overlay, overlaySx]}
      sx={[styles.content, sx]}
    />
  );
});

export function GlassDialogCancelButton({
  className,
  sx,
  ...props
}: ComponentProps<typeof Button>) {
  return (
    <Button
      {...props}
      variant="ghost"
      className={className}
      sx={[styles.cancelButton, sx]}
    />
  );
}

function setForwardedRef<T>(ref: ForwardedRef<T>, value: T | null) {
  if (typeof ref === "function") {
    ref(value);
  } else if (ref) {
    ref.current = value;
  }
}

const styles = stylex.create({
  cancelButton: {
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.background} 50%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.background} 80%, transparent)`,
    },
    borderColor: `color-mix(in oklab, ${colors.border} 70%, transparent)`,
    borderRadius: "9999px",
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 1px 2px rgba(0, 0, 0, 0.06)",
    color: colors.foreground,
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    paddingInline: "1rem",
  },
  content: {
    backdropFilter: "blur(40px) saturate(1.5)",
    backgroundColor: `color-mix(in oklab, ${colors.card} 60%, transparent)`,
    borderColor: `color-mix(in oklab, ${colors.border} 45%, transparent)`,
    borderRadius: {
      default: "26px",
      [media.sm]: "26px",
    },
    boxShadow:
      "inset 0 1px 0 rgba(255, 255, 255, 0.4), 0 24px 70px rgba(0, 0, 0, 0.32)",
    gap: "1rem",
    maxWidth: "320px",
    overflow: "hidden",
    padding: "1.25rem",
    width: "calc(100vw - 48px)",
  },
  overlay: {
    backgroundColor: "rgb(0 0 0 / 0.4)",
  },
});

export { styles as glassDialogStyles };
