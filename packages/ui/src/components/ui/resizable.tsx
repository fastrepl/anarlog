import { DotsSixVertical } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import * as ResizablePrimitive from "react-resizable-panels";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

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
  const handleStyle = mergeStyleXProps([styles.handle, sx], className, style);
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
    alignItems: "center",
    backgroundColor: colors.border,
    bottom: {
      default: null,
      ":is(*)::after": 0,
      ":is([data-panel-group-direction='vertical'])::after": "auto",
    },
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 1px ${colors.background}, 0 0 0 2px ${colors.ring}`,
    },
    content: {
      default: null,
      ":is(*)::after": "''",
    },
    cursor: {
      default: "col-resize",
      ":is([data-panel-group-direction='vertical'])": "row-resize",
    },
    display: "flex",
    height: {
      default: null,
      ":is([data-panel-group-direction='vertical'])": "1px",
      ":is([data-panel-group-direction='vertical'])::after": "0.25rem",
    },
    justifyContent: "center",
    left: {
      default: null,
      ":is(*)::after": "50%",
      ":is([data-panel-group-direction='vertical'])::after": 0,
    },
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
    position: {
      default: "relative",
      ":is(*)::after": "absolute",
    },
    touchAction: "none",
    top: {
      default: null,
      ":is(*)::after": 0,
      ":is([data-panel-group-direction='vertical'])::after": "50%",
    },
    transform: {
      default: null,
      ":is(*)::after": "translateX(-50%)",
      ":is([data-panel-group-direction='vertical'])::after": "translateY(-50%)",
      ":is([data-panel-group-direction='vertical']) > div": "rotate(90deg)",
    },
    width: {
      default: "1px",
      ":is(*)::after": "0.25rem",
      ":is([data-panel-group-direction='vertical'])": "100%",
      ":is([data-panel-group-direction='vertical'])::after": "100%",
    },
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
