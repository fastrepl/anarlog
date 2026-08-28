import { Waveform } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Spinner } from "@anlg/ui/components/ui/spinner";

export function TranscriptListeningState({
  status,
}: {
  status: "listening" | "finalizing";
}) {
  const isFinalizing = status === "finalizing";

  return (
    <div role="status" {...stylex.props(styles.root)}>
      {isFinalizing ? (
        <div {...stylex.props(styles.icon)}>
          <Spinner size={36} />
        </div>
      ) : (
        <Waveform aria-hidden {...stylex.props(styles.icon, styles.waveform)} />
      )}
      <div {...stylex.props(styles.copy)}>
        <p {...stylex.props(styles.title)}>
          {isFinalizing ? "Finalizing transcript..." : "Listening..."}
        </p>
        <p {...stylex.props(styles.description)}>
          {isFinalizing
            ? "Transcript is still being written."
            : "Transcript will appear here when the first segment arrives."}
        </p>
      </div>
    </div>
  );
}

const styles = stylex.create({
  copy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    maxWidth: "28rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    lineHeight: 1.625,
  },
  icon: {
    color: colors.mutedForeground,
    marginBottom: "1.25rem",
  },
  root: {
    alignItems: "center",
    display: "flex",
    flexDirection: "column",
    height: "100%",
    justifyContent: "center",
    minHeight: "400px",
    paddingInline: "1.5rem",
    textAlign: "center",
  },
  title: {
    fontSize: "1rem",
    fontWeight: 500,
  },
  waveform: {
    height: "2.25rem",
    strokeWidth: 1.5,
    width: "2.25rem",
  },
});
