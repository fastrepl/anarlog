import { t } from "@lingui/core/macro";
import { CircleNotch, Waveform } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { Switch } from "@anlg/ui/components/ui/switch";

import {
  isAttachmentShareable,
  type SessionShareAttachment,
} from "./attachments";

export function SessionAttachmentControls({
  attachments,
  sharedAttachmentIds,
  canShare,
  pendingAttachmentId,
  onShareChange,
}: {
  attachments: SessionShareAttachment[];
  sharedAttachmentIds: Map<string, string>;
  canShare: boolean;
  pendingAttachmentId: string | null;
  onShareChange: (
    attachment: SessionShareAttachment,
    included: boolean,
  ) => void;
}) {
  const audio = attachments.find(
    (attachment) => attachment.sourceType === "session_audio",
  );
  if (!audio) return null;

  const included = sharedAttachmentIds.has(audio.id);
  if (!included && audio.localAvailability !== "present") return null;

  const pending = pendingAttachmentId === audio.id;
  const available = isAttachmentShareable(audio);

  return (
    <section {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.row)}>
        <div {...stylex.props(styles.details)}>
          <div {...stylex.props(styles.iconContainer)}>
            <Waveform {...stylex.props(styles.icon)} aria-hidden="true" />
          </div>
          <div {...stylex.props(styles.copy)}>
            <label
              htmlFor={`share-audio-${audio.id}`}
              {...stylex.props(styles.label)}
            >
              {t`Share audio`}
            </label>
            <p {...stylex.props(styles.description)}>
              {available || included
                ? t`Let people with access play the recording.`
                : t`Audio is not available on this device.`}
            </p>
          </div>
        </div>
        {pending ? (
          <CircleNotch
            aria-label={t`Updating audio sharing`}
            {...stylex.props(styles.spinner)}
          />
        ) : (
          <Switch
            id={`share-audio-${audio.id}`}
            size="sm"
            checked={included}
            disabled={!canShare || (!included && !available)}
            onCheckedChange={(checked) => onShareChange(audio, checked)}
          />
        )}
      </div>
    </section>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  copy: {
    minWidth: 0,
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
  },
  details: {
    alignItems: "center",
    display: "flex",
    gap: "0.625rem",
    minWidth: 0,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  iconContainer: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.lg,
    display: "flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    width: "1.75rem",
  },
  label: {
    fontSize: "0.75rem",
    fontWeight: 500,
  },
  root: {
    borderTopColor: `color-mix(in srgb, ${colors.border} 60%, transparent)`,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingBlock: "0.75rem",
  },
  row: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
});
