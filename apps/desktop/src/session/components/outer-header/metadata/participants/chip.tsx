import { Trans, useLingui } from "@lingui/react/macro";
import { Loader2Icon, SparklesIcon, X } from "lucide-react";
import { useCallback, useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

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

  const { humanName } = details;
  const isEnhancing = enhancingHumanId === assignedHumanId;
  const canEnhance = Boolean(onEnhanceContact && assignedHumanId);

  return (
    <Badge
      variant="secondary"
      className={cn([
        "bg-foreground/10 hover:bg-foreground/15 relative flex cursor-pointer items-center gap-1 overflow-hidden px-2 py-0.5 text-xs",
        isEnhancing && "ring-ring/20 ring-1",
      ])}
      onClick={handleClick}
    >
      {isEnhancing && (
        <span
          aria-hidden="true"
          className="animate-shimmer pointer-events-none absolute inset-0 -translate-x-full bg-linear-to-r from-transparent via-white/60 to-transparent"
        />
      )}
      <span className="relative z-10">{humanName || "Unknown"}</span>
      {canEnhance && (
        <EnhanceContactButton
          isEnhancing={isEnhancing}
          isDisabled={Boolean(enhancingHumanId)}
          label={humanName || "contact"}
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
        className="relative z-10 ml-0.5 h-3 w-3 p-0 hover:bg-transparent"
        onClick={(e) => {
          e.stopPropagation();
          handleRemove();
        }}
      >
        <X className="h-2.5 w-2.5" />
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
          className="text-muted-foreground hover:text-foreground relative z-10 ml-0.5 h-3.5 w-3.5 p-0 hover:bg-transparent"
          disabled={isDisabled}
          onClick={(e) => {
            e.stopPropagation();
            onClick();
          }}
        >
          {isEnhancing ? (
            <Loader2Icon className="h-2.5 w-2.5 animate-spin" />
          ) : (
            <SparklesIcon className="h-2.5 w-2.5" />
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
