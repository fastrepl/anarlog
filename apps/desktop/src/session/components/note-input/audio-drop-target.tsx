import { Waveform } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { HTMLAttributes, ReactNode } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import { AUDIO_EXTENSIONS } from "~/stt/useUploadFile";

const supportedAudioFormats = formatAudioExtensionList(AUDIO_EXTENSIONS);

export function AudioDropTarget({
  children,
  isActive,
  sx,
  targetProps,
}: {
  children: ReactNode;
  isActive: boolean;
  targetProps: HTMLAttributes<HTMLDivElement>;
} & StyleXProps) {
  return (
    <div
      {...targetProps}
      {...mergeStyleXProps(
        [styles.root, sx],
        targetProps.className,
        targetProps.style,
      )}
    >
      {isActive && (
        <div role="status" {...stylex.props(styles.overlay)}>
          <div {...stylex.props(styles.message)}>
            <Waveform {...stylex.props(styles.icon)} />
            <div {...stylex.props(styles.copy)}>
              <p {...stylex.props(styles.title)}>
                Drop to upload and transcribe audio
              </p>
              <p {...stylex.props(styles.description)}>
                {supportedAudioFormats} audio
              </p>
            </div>
          </div>
        </div>
      )}
      {children}
    </div>
  );
}

function formatAudioExtensionList(extensions: string[]) {
  const labels = extensions.map((extension) => extension.toUpperCase());
  if (labels.length <= 1) {
    return labels.join("");
  }

  return `${labels.slice(0, -1).join(", ")}, or ${labels[labels.length - 1]}`;
}

const styles = stylex.create({
  copy: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
    minWidth: 0,
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  icon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1.25rem",
    width: "1.25rem",
  },
  message: {
    alignItems: "center",
    backgroundColor: `color-mix(in srgb, ${colors.card} 95%, transparent)`,
    borderColor: `color-mix(in srgb, ${colors.border} 70%, transparent)`,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.sm,
    color: colors.foreground,
    display: "flex",
    gap: "0.75rem",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  overlay: {
    alignItems: "center",
    backgroundColor: colors.background,
    backgroundImage:
      "radial-gradient(circle at center, rgba(113, 113, 122, 0.34) 1px, transparent 1px)",
    backgroundSize: "18px 18px",
    borderColor: `color-mix(in srgb, ${colors.border} 70%, transparent)`,
    borderRadius: radii.lg,
    borderStyle: "dashed",
    borderWidth: "1px",
    boxShadow: "inset 0 2px 4px 0 rgb(0 0 0 / 0.05)",
    color: colors.mutedForeground,
    display: "flex",
    inset: 0,
    justifyContent: "center",
    pointerEvents: "none",
    position: "absolute",
    zIndex: 30,
  },
  root: {
    minHeight: "100%",
    position: "relative",
  },
  title: {
    fontSize: "0.875rem",
    fontWeight: 500,
  },
});

export { styles as audioDropTargetStyles };
