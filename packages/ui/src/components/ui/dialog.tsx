import { X } from "@phosphor-icons/react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import {
  colors,
  media,
  radii,
  shadows,
} from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const Dialog = DialogPrimitive.Root;
const DialogTrigger = DialogPrimitive.Trigger;
const DialogPortal = DialogPrimitive.Portal;
const DialogClose = DialogPrimitive.Close;

type DialogContentProps = React.ComponentPropsWithoutRef<
  typeof DialogPrimitive.Content
> &
  StyleXProps & {
    showOverlay?: boolean;
    overlayClassName?: string;
    overlayChildren?: React.ReactNode;
    overlaySx?: StyleXProps["sx"];
  };

const DialogOverlay = React.forwardRef<
  React.ComponentRef<typeof DialogPrimitive.Overlay>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay> & StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <DialogPrimitive.Overlay
    {...props}
    {...mergeStyleXProps([styles.overlay, sx], className, style)}
    data-dialog-overlay
    ref={ref}
  />
));
DialogOverlay.displayName = DialogPrimitive.Overlay.displayName;

const DialogContent = React.forwardRef<
  React.ComponentRef<typeof DialogPrimitive.Content>,
  DialogContentProps
>(
  (
    {
      className,
      children,
      showOverlay = true,
      overlayClassName,
      overlayChildren,
      overlaySx,
      style,
      sx,
      ...props
    },
    ref,
  ) => (
    <DialogPortal>
      {showOverlay ? (
        <DialogPrimitive.Overlay
          {...mergeStyleXProps([styles.overlay, overlaySx], overlayClassName)}
          data-dialog-overlay
        >
          {overlayChildren}
        </DialogPrimitive.Overlay>
      ) : null}
      <DialogPrimitive.Content
        {...props}
        {...mergeStyleXProps([styles.content, sx], className, style)}
        ref={ref}
      >
        {children}
        <DialogPrimitive.Close
          {...stylex.props(styles.close, styles.focusOutlineHidden)}
        >
          <X {...stylex.props(styles.icon)} />
          <span {...stylex.props(styles.visuallyHidden)}>Close</span>
        </DialogPrimitive.Close>
      </DialogPrimitive.Content>
    </DialogPortal>
  ),
);
DialogContent.displayName = DialogPrimitive.Content.displayName;

const DialogHeader = ({
  className,
  style,
  sx,
  ...props
}: React.HTMLAttributes<HTMLDivElement> & StyleXProps) => (
  <div
    {...props}
    {...mergeStyleXProps([styles.header, sx], className, style)}
  />
);
DialogHeader.displayName = "DialogHeader";

const DialogFooter = ({
  className,
  style,
  sx,
  ...props
}: React.HTMLAttributes<HTMLDivElement> & StyleXProps) => (
  <div
    {...props}
    {...mergeStyleXProps([styles.footer, sx], className, style)}
  />
);
DialogFooter.displayName = "DialogFooter";

const DialogTitle = React.forwardRef<
  React.ComponentRef<typeof DialogPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Title> & StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <DialogPrimitive.Title
    {...props}
    {...mergeStyleXProps([styles.title, sx], className, style)}
    ref={ref}
  />
));
DialogTitle.displayName = DialogPrimitive.Title.displayName;

const DialogDescription = React.forwardRef<
  React.ComponentRef<typeof DialogPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Description> &
    StyleXProps
>(({ className, style, sx, ...props }, ref) => (
  <DialogPrimitive.Description
    {...props}
    {...mergeStyleXProps([styles.description, sx], className, style)}
    ref={ref}
  />
));
DialogDescription.displayName = DialogPrimitive.Description.displayName;

const contentEnter = stylex.keyframes({
  from: {
    opacity: 0,
    transform: "translate3d(-50%, -48%, 0) scale3d(0.95, 0.95, 0.95)",
  },
});

const contentExit = stylex.keyframes({
  to: {
    opacity: 0,
    transform: "translate3d(-50%, -48%, 0) scale3d(0.95, 0.95, 0.95)",
  },
});

const overlayEnter = stylex.keyframes({
  from: {
    opacity: 0,
  },
});

const overlayExit = stylex.keyframes({
  to: {
    opacity: 0,
  },
});

const styles = stylex.create({
  close: {
    backgroundColor: {
      default: null,
      ':is([data-state="open"])': colors.accent,
    },
    borderRadius: "0.125rem",
    boxShadow: {
      default: null,
      ":focus": `0 0 0 2px ${colors.background}, 0 0 0 4px ${colors.ring}`,
    },
    color: {
      default: null,
      ':is([data-state="open"])': colors.mutedForeground,
    },
    opacity: {
      default: 0.7,
      ":hover": {
        default: null,
        "@media (hover: hover)": 1,
      },
    },
    pointerEvents: {
      default: null,
      ":disabled": "none",
    },
    position: "absolute",
    right: "1rem",
    top: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  content: {
    animationDuration: {
      default: null,
      ':is([data-state="closed"])': "200ms",
      ':is([data-state="open"])': "200ms",
    },
    animationName: {
      default: null,
      ':is([data-state="closed"])': contentExit,
      ':is([data-state="open"])': contentEnter,
    },
    backgroundColor: "hsl(var(--background, 0 0% 100%))",
    borderColor: colors.border,
    borderRadius: {
      default: 0,
      [media.sm]: radii.lg,
    },
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.lg,
    display: "grid",
    gap: "1rem",
    left: "50%",
    maxWidth: "32rem",
    padding: "1.5rem",
    position: "fixed",
    top: "50%",
    transform: "translate(-50%, -50%)",
    transitionDuration: "200ms",
    width: "100%",
    zIndex: 50,
  },
  focusOutlineHidden: {
    outlineColor: {
      default: null,
      ":focus": {
        default: null,
        "@media (forced-colors: active)": "transparent",
      },
    },
    outlineOffset: {
      default: null,
      ":focus": {
        default: null,
        "@media (forced-colors: active)": "2px",
      },
    },
    outlineStyle: {
      default: null,
      ":focus": {
        default: "none",
        "@media (forced-colors: active)": "solid",
      },
    },
    outlineWidth: {
      default: null,
      ":focus": {
        default: null,
        "@media (forced-colors: active)": "2px",
      },
    },
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
  footer: {
    display: "flex",
    flexDirection: {
      default: "column-reverse",
      [media.sm]: "row",
    },
    gap: "0.5rem",
    justifyContent: {
      default: null,
      [media.sm]: "flex-end",
    },
  },
  header: {
    display: "flex",
    flexDirection: "column",
    gap: "0.375rem",
    textAlign: {
      default: "center",
      [media.sm]: "left",
    },
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  overlay: {
    animationDuration: {
      default: null,
      ':is([data-state="closed"])': "150ms",
      ':is([data-state="open"])': "150ms",
    },
    animationName: {
      default: null,
      ':is([data-state="closed"])': overlayExit,
      ':is([data-state="open"])': overlayEnter,
    },
    backgroundColor: "rgb(0 0 0 / 0.8)",
    inset: 0,
    position: "fixed",
    zIndex: 50,
  },
  title: {
    fontSize: "1.125rem",
    fontWeight: 600,
    letterSpacing: "-0.025em",
    lineHeight: 1,
  },
  visuallyHidden: {
    borderWidth: 0,
    clipPath: "inset(50%)",
    height: "1px",
    margin: "-1px",
    overflow: "hidden",
    padding: 0,
    position: "absolute",
    whiteSpace: "nowrap",
    width: "1px",
  },
});

export {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogOverlay,
  DialogPortal,
  DialogTitle,
  DialogTrigger,
};
