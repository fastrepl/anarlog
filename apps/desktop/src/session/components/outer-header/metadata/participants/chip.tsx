import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch, Sparkle, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useState } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Badge } from "@anlg/ui/components/ui/badge";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@anlg/ui/components/ui/tooltip";

import {
  removeSessionParticipant,
  useSessionParticipant,
} from "~/session/queries";
import { useTabs } from "~/store/zustand/tabs/index";
import { removeHumanSpeakerAssignments } from "~/stt/queries";

export function ParticipantChip({
  mappingId,
  enhancingHumanId,
  onEnhanceContact,
}: {
  mappingId: string;
  enhancingHumanId?: string;
  onEnhanceContact?: (humanId: string) => void;
}) {
  const details = useParticipantDetails(mappingId);

  const assignedHumanId = details?.humanId;
  const sessionId = details?.sessionId;
  const source = details?.source;

  const { remove: handleRemove, isRemoving } = useRemoveParticipant({
    mappingId,
    assignedHumanId,
    sessionId,
  });

  const handleClick = useCallback(() => {
    if (assignedHumanId) {
      useTabs.getState().openNew({
        type: "contacts",
        state: { selected: { type: "person", id: assignedHumanId } },
      });
    }
  }, [assignedHumanId]);

  if (!details || source === "excluded" || isRemoving) {
    return null;
  }

  const { humanName, humanEmail } = details;
  const displayName = humanName.trim() || humanEmail?.trim();

  if (!displayName) {
    return null;
  }

  const isEnhancing = enhancingHumanId === assignedHumanId;
  const canEnhance = Boolean(onEnhanceContact && assignedHumanId);

  return (
    <Badge
      variant="secondary"
      sx={[styles.badge, isEnhancing && styles.enhancingBadge]}
      onClick={handleClick}
    >
      {isEnhancing && (
        <span aria-hidden="true" {...stylex.props(styles.shimmer)} />
      )}
      <span {...stylex.props(styles.label)}>{displayName}</span>
      {canEnhance && (
        <EnhanceContactButton
          isEnhancing={isEnhancing}
          isDisabled={Boolean(enhancingHumanId)}
          label={displayName}
          onClick={() => {
            if (assignedHumanId) {
              onEnhanceContact?.(assignedHumanId);
            }
          }}
        />
      )}
      <Button
        type="button"
        variant="ghost"
        size="sm"
        sx={styles.removeButton}
        onClick={(e) => {
          e.stopPropagation();
          handleRemove();
        }}
      >
        <X {...stylex.props(styles.smallIcon)} />
      </Button>
    </Badge>
  );
}

function EnhanceContactButton({
  isEnhancing,
  isDisabled,
  label,
  onClick,
}: {
  isEnhancing: boolean;
  isDisabled: boolean;
  label: string;
  onClick: () => void;
}) {
  const { t } = useLingui();
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-label={t`Enhance contact ${label}`}
          sx={styles.enhanceButton}
          disabled={isDisabled}
          onClick={(e) => {
            e.stopPropagation();
            onClick();
          }}
        >
          {isEnhancing ? (
            <CircleNotch {...stylex.props(styles.smallIcon, styles.spinner)} />
          ) : (
            <Sparkle {...stylex.props(styles.smallIcon)} />
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <Trans>Enhance contact</Trans>
      </TooltipContent>
    </Tooltip>
  );
}

function useParticipantDetails(mappingId: string) {
  const participant = useSessionParticipant(mappingId);

  if (!participant) {
    return null;
  }

  return {
    mappingId,
    humanId: participant.humanId,
    humanName: participant.name,
    humanEmail: participant.email || undefined,
    humanJobTitle: participant.jobTitle || undefined,
    humanLinkedinUsername: participant.linkedinUsername || undefined,
    orgId: participant.organizationId || undefined,
    orgName: participant.organizationName || undefined,
    sessionId: participant.sessionId,
    source: participant.source,
  };
}

function useRemoveParticipant({
  mappingId,
  assignedHumanId,
  sessionId,
}: {
  mappingId: string;
  assignedHumanId: string | undefined;
  sessionId: string | undefined;
}) {
  const [isRemoving, setIsRemoving] = useState(false);

  const remove = useCallback(() => {
    setIsRemoving(true);
    void (async () => {
      if (assignedHumanId && sessionId) {
        await removeHumanSpeakerAssignments(sessionId, assignedHumanId);
      }
      await removeSessionParticipant(mappingId);
    })().catch((error) => {
      setIsRemoving(false);
      console.error("[participants] failed to remove participant", error);
    });
  }, [mappingId, assignedHumanId, sessionId]);

  return { remove, isRemoving };
}

const shimmer = stylex.keyframes({
  from: {
    transform: "translateX(-100%)",
  },
  to: {
    transform: "translateX(100%)",
  },
});

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  badge: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in srgb, ${colors.foreground} 10%, transparent)`,
      ":hover": `color-mix(in srgb, ${colors.foreground} 15%, transparent)`,
    },
    cursor: "pointer",
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    overflow: "hidden",
    paddingBlock: "0.125rem",
    paddingInline: "0.5rem",
    position: "relative",
  },
  enhanceButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": "transparent",
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    height: "0.875rem",
    marginLeft: "0.125rem",
    padding: 0,
    position: "relative",
    width: "0.875rem",
  },
  enhancingBadge: {
    boxShadow: `0 0 0 1px color-mix(in srgb, ${colors.ring} 20%, transparent)`,
  },
  label: {
    position: "relative",
  },
  removeButton: {
    backgroundColor: {
      default: "transparent",
      ":hover": "transparent",
    },
    height: "0.75rem",
    marginLeft: "0.125rem",
    padding: 0,
    position: "relative",
    width: "0.75rem",
  },
  shimmer: {
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: shimmer,
    animationTimingFunction: "linear",
    backgroundImage:
      "linear-gradient(to right, transparent, rgb(255 255 255 / 0.6), transparent)",
    inset: 0,
    pointerEvents: "none",
    position: "absolute",
    transform: "translateX(-100%)",
  },
  smallIcon: {
    height: "0.625rem",
    width: "0.625rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
});
