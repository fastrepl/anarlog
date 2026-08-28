import * as stylex from "@stylexjs/stylex";

export function StandaloneWindowShell({
  children,
  topDragRegion = true,
}: {
  children: React.ReactNode;
  topDragRegion?: boolean;
}) {
  return (
    <div {...stylex.props(styles.root)}>
      {topDragRegion ? (
        <div
          data-tauri-drag-region
          data-standalone-window-top-drag-region
          {...stylex.props(styles.dragRegion)}
        />
      ) : null}
      {children}
    </div>
  );
}

const styles = stylex.create({
  dragRegion: {
    height: "2.5rem",
    left: 0,
    position: "absolute",
    right: 0,
    top: 0,
    zIndex: 20,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    position: "relative",
  },
});
