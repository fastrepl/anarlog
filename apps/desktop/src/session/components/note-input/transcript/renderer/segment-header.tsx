import { Check } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { useTranscriptSelectionState } from "./selection-context";
import { SpeakerAssignPopover } from "./speaker-assign";
import { useSegmentColorVars } from "./utils";

import type { Segment } from "~/stt/live-segment";

export function SegmentHeader({
  segment,
  transcriptId,
  sessionId,
  label,
  selected = false,
}: {
  segment: Segment;
  transcriptId: string;
  sessionId?: string;
  label: string;
  selected?: boolean;
}) {
  const { selectMode } = useTranscriptSelectionState();
  const colorVars = useSegmentColorVars(segment.key);

  return (
    <div {...mergeStyleXProps(styles.root, undefined, colorVars)}>
      {selectMode ? (
        <span
          aria-hidden="true"
          {...stylex.props(
            styles.selection,
            selected ? styles.selectionActive : styles.selectionInactive,
          )}
        >
          {selected ? (
            <Check {...stylex.props(styles.check)} weight="bold" />
          ) : null}
        </span>
      ) : null}
      <SpeakerAssignPopover
        segment={segment}
        transcriptId={transcriptId}
        sessionId={sessionId}
        color="light-dark(var(--segment-color-light), var(--segment-color-dark))"
        label={label}
      />
    </div>
  );
}

const styles = stylex.create({
  check: {
    height: "0.625rem",
    width: "0.625rem",
  },
  root: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 300,
    gap: "0.5rem",
    paddingBlock: "0.25rem",
    position: "relative",
  },
  selection: {
    alignItems: "center",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexShrink: 0,
    height: "1rem",
    justifyContent: "center",
    width: "1rem",
  },
  selectionActive: {
    backgroundColor: colors.primary,
    borderColor: colors.primary,
    color: colors.primaryForeground,
  },
  selectionInactive: {
    borderColor: `color-mix(in srgb, ${colors.mutedForeground} 40%, transparent)`,
  },
});

export { styles as segmentHeaderStyles };
