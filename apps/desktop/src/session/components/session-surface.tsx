import * as stylex from "@stylexjs/stylex";

import { StandardContentWrapper } from "~/shared/main";

export function SessionSurface({
  header,
  children,
  floatingButton,
  overlay,
}: {
  header?: React.ReactNode;
  children: React.ReactNode;
  floatingButton?: React.ReactNode;
  overlay?: React.ReactNode;
}) {
  return (
    <StandardContentWrapper floatingButton={floatingButton}>
      <div data-session-surface {...stylex.props(styles.root)}>
        <div
          {...stylex.props(
            styles.shell,
            Boolean(overlay) && styles.shellWithOverlay,
          )}
          {...(overlay ? { inert: true, "aria-hidden": true } : {})}
        >
          <div
            {...stylex.props(
              styles.shell,
              Boolean(overlay) && styles.blurredContent,
            )}
          >
            {header ? (
              <div data-tauri-drag-region {...stylex.props(styles.header)}>
                {header}
              </div>
            ) : null}
            <div {...stylex.props(styles.content)}>{children}</div>
          </div>
        </div>
        {overlay ? (
          <div {...stylex.props(styles.overlay)}>{overlay}</div>
        ) : null}
      </div>
    </StandardContentWrapper>
  );
}

const styles = stylex.create({
  blurredContent: {
    filter: "blur(22px)",
    transform: "scale(1.03)",
    transformOrigin: "center",
    userSelect: "none",
  },
  content: {
    flex: "1",
    minHeight: 0,
    paddingInline: "0.5rem",
  },
  header: {
    paddingInline: "0.25rem",
  },
  overlay: {
    inset: 0,
    overflow: "hidden",
    position: "absolute",
    zIndex: 10,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    isolation: "isolate",
    position: "relative",
  },
  shell: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    height: "100%",
    minHeight: 0,
  },
  shellWithOverlay: {
    overflow: "hidden",
    position: "relative",
    zIndex: 0,
  },
});

export { styles as sessionSurfaceStyles };
