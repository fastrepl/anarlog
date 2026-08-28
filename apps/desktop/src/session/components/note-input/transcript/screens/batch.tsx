import * as stylex from "@stylexjs/stylex";

import { colors } from "@anlg/design-system/tokens.stylex";
import type { DegradedError } from "@anlg/plugin-transcription";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";

import { useListener } from "~/stt/contexts";

export function BatchState({
  requestedLiveTranscription,
  error,
}: {
  requestedLiveTranscription: boolean | null;
  error: DegradedError | null;
}) {
  const amplitude = useListener((state) => state.live.amplitude);
  const isFallbackFromLive = requestedLiveTranscription === true;
  const isReconnecting = isFallbackFromLive && isRetryable(error);
  const title = isFallbackFromLive
    ? isReconnecting
      ? "Reconnecting live transcription"
      : error
        ? "Live transcription stopped"
        : "Live transcription unavailable"
    : "Batch transcription mode";
  const description = isFallbackFromLive
    ? `${error ? `${degradedMessage(error)}. ` : ""}Recording continues${
        isReconnecting ? " while we reconnect" : ""
      }. A complete transcript will be generated after you stop.`
    : "Recording continues. Your transcript will be generated after you stop.";

  return (
    <div role="status" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.visualizer)}>
        <DancingSticks
          amplitude={Math.min(Math.hypot(amplitude.mic, amplitude.speaker), 1)}
          color="#a3a3a3"
          height={36}
          width={80}
          stickWidth={3}
          gap={3}
        />
      </div>
      <div {...stylex.props(styles.copy)}>
        <p {...stylex.props(styles.title)}>{title}</p>
        <p {...stylex.props(styles.description)}>{description}</p>
      </div>
    </div>
  );
}

function isRetryable(error: DegradedError | null) {
  return (
    error !== null &&
    error.type !== "authentication_failed" &&
    error.type !== "provider_configuration"
  );
}

function degradedMessage(error: DegradedError): string {
  switch (error.type) {
    case "authentication_failed":
      return `Authentication failed (${error.provider})`;
    case "upstream_unavailable":
      return error.message;
    case "connection_timeout":
      return "Transcription connection timed out";
    case "provider_configuration":
      return `Transcription provider is misconfigured (${error.provider})`;
    case "stream_error":
      return "Transcription stream error";
  }
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
  visualizer: {
    marginBottom: "1.25rem",
  },
});
