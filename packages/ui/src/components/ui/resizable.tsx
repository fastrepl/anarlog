import { DotsSixVertical } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import * as ResizablePrimitive from "react-resizable-panels";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";
import { cn } from "@anlg/utils";

const ResizablePanelGroup = ({
  className,
  style,
  sx,
  direction,
  ...props
}: React.ComponentProps<typeof ResizablePrimitive.PanelGroup> &
  StyleXProps) => {
  const resolvedStyle = mergeStyleXProps(
    [
      styles.panelGroup,
      direction === "vertical" && styles.panelGroupVertical,
      sx,
    ],
    className,
    style,
  );

  return (
    <ResizablePrimitive.PanelGroup
      direction={direction}
      {...props}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
};

const ResizablePanel = ({
  className,
  style,
  sx,
  ...props
}: React.ComponentProps<typeof ResizablePrimitive.Panel> & StyleXProps) => {
  const resolvedStyle = mergeStyleXProps([styles.panel, sx], className, style);

  return (
    <ResizablePrimitive.Panel
      {...props}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
};

const ResizableHandle = ({
  withHandle,
  className,
  style,
  sx,
  ...props
}: React.ComponentProps<typeof ResizablePrimitive.PanelResizeHandle> & {
  withHandle?: boolean;
} & StyleXProps) => {
  const handleStyle = mergeStyleXProps(
    [styles.handle, sx],
    cn([resizableHandleSelectorClassName, className]),
    style,
  );
  const gripStyle = mergeStyleXProps(styles.grip);
  const iconStyle = mergeStyleXProps(styles.gripIcon);

  return (
    <ResizablePrimitive.PanelResizeHandle
      {...props}
      className={handleStyle.className}
      style={handleStyle.style}
    >
      {withHandle && (
        <div className={gripStyle.className} style={gripStyle.style}>
          <DotsSixVertical
            className={iconStyle.className}
            style={iconStyle.style}
          />
        </div>
      )}
    </ResizablePrimitive.PanelResizeHandle>
  );
};

const resizableHandleSelectorClassName =
  "data-[panel-group-direction=vertical]:h-px data-[panel-group-direction=vertical]:w-full data-[panel-group-direction=vertical]:cursor-row-resize data-[panel-group-direction=vertical]:after:left-0 data-[panel-group-direction=vertical]:after:h-1 data-[panel-group-direction=vertical]:after:w-full data-[panel-group-direction=vertical]:after:translate-x-0 data-[panel-group-direction=vertical]:after:-translate-y-1/2 [&[data-panel-group-direction=vertical]>div]:rotate-90";

const styles = stylex.create({
  panelGroup: {
    display: "flex",
    height: "100%",
    minHeight: 0,
    minWidth: 0,
    width: "100%",
  },
  panelGroupVertical: {
    flexDirection: "column",
  },
  panel: {
    minHeight: 0,
    minWidth: 0,
  },
  handle: {
    "::after": {
      bottom: 0,
      content: "''",
      left: "50%",
      position: "absolute",
      top: 0,
      transform: "translateX(-50%)",
      width: "0.25rem",
    },
    alignItems: "center",
    backgroundColor: colors.border,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 1px ${colors.background}, 0 0 0 2px ${colors.ring}`,
    },
    cursor: "col-resize",
    display: "flex",
    justifyContent: "center",
    outlineColor: {
      default: null,
      ":focus-visible": "transparent",
    },
    outlineOffset: {
      default: null,
      ":focus-visible": "2px",
    },
    outlineStyle: {
      default: null,
      ":focus-visible": "solid",
    },
    outlineWidth: {
      default: null,
      ":focus-visible": "2px",
    },
    position: "relative",
    touchAction: "none",
    width: "1px",
  },
  grip: {
    alignItems: "center",
    backgroundColor: colors.border,
    borderColor: colors.border,
    borderRadius: "0.125rem",
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    height: "1rem",
    justifyContent: "center",
    position: "relative",
    width: "0.75rem",
    zIndex: 10,
  },
  gripIcon: {
    height: "0.625rem",
    width: "0.625rem",
  },
});

export type { ImperativePanelHandle } from "react-resizable-panels";
export { ResizableHandle, ResizablePanel, ResizablePanelGroup };
