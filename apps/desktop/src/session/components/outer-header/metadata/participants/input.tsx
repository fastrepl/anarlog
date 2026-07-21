import {
  autoUpdate,
  flip,
  FloatingPortal,
  offset,
  shift,
  size,
  useFloating,
} from "@floating-ui/react";
import { useMutation } from "@tanstack/react-query";
import { Loader2Icon } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { ParticipantChip } from "./chip";
import { ParticipantDropdown } from "./dropdown";
import {
  buildEventContactExtractionContextFromRecords,
  extractEventContacts,
  planExtractedContactToHuman,
} from "./event-contact-extraction";

import { useLanguageModel } from "~/ai/hooks";
import { useSessionEventParticipants } from "~/calendar/queries";
import {
  applyContactEnhancement,
  createHuman,
  useHumans,
} from "~/contacts/queries";
import {
  addSessionParticipant,
  removeSessionParticipant,
  useSession,
  useSessionParticipants,
} from "~/session/queries";
import { getSessionEvent } from "~/session/utils";
import { useAutoCloser } from "~/shared/hooks/useAutoCloser";
import { removeHumanSpeakerAssignments } from "~/stt/queries";

export function ParticipantInput({ sessionId }: { sessionId: string }) {
  const {
    inputValue,
    showDropdown,
    setShowDropdown,
    selectedIndex,
    setSelectedIndex,
    mappingIds,
    pendingParticipants,
    dropdownOptions,
    handleAddParticipant,
    handleInputChange,
    deleteLastParticipant,
    resetInput,
  } = useParticipantInput(sessionId);
  const { enhanceContact, enhancingHumanId, showEnhancementButtons } =
    useEventContactEnhancement(sessionId);
  const placeholder = "Add participants";

  const inputRef = useRef<HTMLInputElement>(null);
  const autoCloserRef = useAutoCloser(() => setShowDropdown(false), {
    esc: false,
    outside: true,
  });
  const { refs, floatingStyles } = useFloating({
    placement: "bottom-start",
    strategy: "fixed",
    whileElementsMounted: autoUpdate,
    middleware: [
      offset(4),
      flip({ padding: 8 }),
      shift({ padding: 8 }),
      size({
        apply({ rects, elements }) {
          Object.assign(elements.floating.style, {
            width: `${rects.reference.width}px`,
          });
        },
      }),
    ],
  });

  const setContainerRef = useCallback(
    (node: HTMLDivElement | null) => {
      autoCloserRef.current = node;
      refs.setReference(node);
    },
    [autoCloserRef, refs],
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if ((e.key === "Enter" || e.key === "Tab") && inputValue.trim()) {
      if (dropdownOptions.length > 0) {
        e.preventDefault();
        handleAddParticipant(dropdownOptions[selectedIndex]);
        inputRef.current?.focus();
      }
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) =>
        prev < dropdownOptions.length - 1 ? prev + 1 : prev,
      );
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev > 0 ? prev - 1 : prev));
    } else if (e.key === "Escape") {
      resetInput();
    } else if (e.key === "Backspace" && !inputValue) {
      deleteLastParticipant();
    }
  };

  const handleSelect = (option: Candidate) => {
    handleAddParticipant(option);
    inputRef.current?.focus();
  };

  return (
    <div className="relative" ref={setContainerRef}>
      <div
        className="flex min-h-[38px] w-full cursor-text flex-wrap items-center gap-2"
        onClick={() => inputRef.current?.focus()}
      >
        {mappingIds.map((mappingId) => (
          <ParticipantChip
            key={mappingId}
            mappingId={mappingId}
            enhancingHumanId={enhancingHumanId}
            onEnhanceContact={
              showEnhancementButtons ? enhanceContact : undefined
            }
          />
        ))}

        {pendingParticipants.map((pending) => (
          <Badge
            key={pending.key}
            variant="secondary"
            className="bg-foreground/10 flex items-center gap-1 px-2 py-0.5 text-xs opacity-60"
          >
            <span>{pending.name || "Unknown"}</span>
            <Loader2Icon className="size-2.5 animate-spin" aria-hidden="true" />
          </Badge>
        ))}

        <input
          ref={inputRef}
          type="text"
          className="placeholder:text-muted-foreground min-w-[120px] flex-1 bg-transparent text-sm outline-hidden"
          placeholder={placeholder}
          value={inputValue}
          onChange={(e) => handleInputChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onFocus={() => setShowDropdown(true)}
        />
      </div>

      {showDropdown && inputValue.trim() && (
        <FloatingPortal>
          <ParticipantDropdown
            floatingRef={refs.setFloating}
            floatingStyles={floatingStyles}
            options={dropdownOptions}
            selectedIndex={selectedIndex}
            onSelect={handleSelect}
            onHover={setSelectedIndex}
          />
        </FloatingPortal>
      )}
    </div>
  );
}

type Candidate = {
  id: string;
  name: string;
  email?: string;
  phone?: string;
  orgId?: string;
  jobTitle?: string;
  isNew?: boolean;
};

function useParticipantMappings(sessionId: string) {
  const participants = useSessionParticipants(sessionId);
  const activeParticipants = useMemo(
    () =>
      participants.filter((participant) => participant.source !== "excluded"),
    [participants],
  );
  const existingHumanIds = useMemo(() => {
    return new Set(
      activeParticipants.map((participant) => participant.humanId),
    );
  }, [activeParticipants]);

  return {
    participants: activeParticipants,
    mappingIds: activeParticipants.map((participant) => participant.id),
    existingHumanIds,
  };
}

function useCandidateSearch(
  inputValue: string,
  existingHumanIds: Set<string>,
): Candidate[] {
  const humans = useHumans();

  return useMemo(() => {
    const searchLower = inputValue.toLowerCase();
    return humans
      .filter((human) => !existingHumanIds.has(human.id))
      .map((human) => {
        const { name, email, phone } = human;
        const nameMatch = name.toLowerCase().includes(searchLower);
        const emailMatch = email.toLowerCase().includes(searchLower);
        const phoneMatch = phone.toLowerCase().includes(searchLower);

        if (inputValue && !nameMatch && !emailMatch && !phoneMatch) {
          return null;
        }

        return {
          id: human.id,
          name,
          email,
          phone,
          orgId: human.organizationId || undefined,
          jobTitle: human.jobTitle || undefined,
          isNew: false,
        };
      })
      .filter((h): h is NonNullable<typeof h> => h !== null);
  }, [inputValue, existingHumanIds, humans]);
}

function useDropdownOptions(
  inputValue: string,
  candidates: Candidate[],
): Candidate[] {
  return useMemo(() => {
    const showCustomOption =
      inputValue.trim() &&
      !candidates.some(
        (c) => c.name.toLowerCase() === inputValue.toLowerCase(),
      );

    if (!showCustomOption) {
      return candidates;
    }

    return [
      {
        id: "new",
        name: inputValue.trim(),
        isNew: true,
        email: "",
        orgId: undefined,
        jobTitle: undefined,
      },
      ...candidates,
    ];
  }, [inputValue, candidates]);
}

const PENDING_ADD_SETTLE_GRACE_MS = 5000;

function useParticipantMutations(
  sessionId: string,
  participants: ReturnType<typeof useSessionParticipants>,
) {
  const session = useSession(sessionId);
  const [pendingAdds, setPendingAdds] = useState<
    { key: number; humanId: string | null; name: string }[]
  >([]);
  const pendingAddKeyRef = useRef(0);

  const addParticipant = useCallback(
    (option: Candidate) => {
      const key = ++pendingAddKeyRef.current;
      setPendingAdds((prev) => [
        ...prev,
        { key, humanId: option.isNew ? null : option.id, name: option.name },
      ]);

      void (async () => {
        let humanId = option.id;
        if (option.isNew) {
          if (!session?.user_id) return;
          humanId = await createHuman({
            ownerUserId: session.user_id,
            name: option.name,
          });
          // Backfill the id so the pending chip disappears as soon as the
          // real participant row shows up, instead of lingering until the
          // finally below and briefly duplicating the chip.
          const resolvedHumanId = humanId;
          setPendingAdds((prev) =>
            prev.map((entry) =>
              entry.key === key
                ? { ...entry, humanId: resolvedHumanId }
                : entry,
            ),
          );
        }
        await addSessionParticipant(sessionId, humanId);
      })().then(
        () => {
          // The pending chip is already hidden once the live query lists the
          // human; delay the removal so a slow re-emit cannot leave a gap
          // between the pending chip and the real one.
          setTimeout(() => {
            setPendingAdds((prev) => prev.filter((entry) => entry.key !== key));
          }, PENDING_ADD_SETTLE_GRACE_MS);
        },
        (error: unknown) => {
          console.error("[participants] failed to add participant", error);
          setPendingAdds((prev) => prev.filter((entry) => entry.key !== key));
        },
      );
    },
    [session?.user_id, sessionId],
  );

  const deleteLastParticipant = useCallback(() => {
    const participant = participants[participants.length - 1];
    if (!participant) return;

    void (async () => {
      await removeHumanSpeakerAssignments(sessionId, participant.humanId);
      await removeSessionParticipant(participant.id);
    })().catch((error) => {
      console.error("[participants] failed to remove participant", error);
    });
  }, [participants, sessionId]);

  return { addParticipant, deleteLastParticipant, pendingAdds };
}

function useParticipantInput(sessionId: string) {
  const [inputValue, setInputValue] = useState("");
  const [showDropdown, setShowDropdown] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const { participants, mappingIds, existingHumanIds } =
    useParticipantMappings(sessionId);
  const candidates = useCandidateSearch(inputValue, existingHumanIds);
  const dropdownOptions = useDropdownOptions(inputValue, candidates);
  const { addParticipant, deleteLastParticipant, pendingAdds } =
    useParticipantMutations(sessionId, participants);
  const pendingParticipants = useMemo(
    () =>
      pendingAdds.filter(
        (entry) => !entry.humanId || !existingHumanIds.has(entry.humanId),
      ),
    [existingHumanIds, pendingAdds],
  );
  const activeSelectedIndex =
    dropdownOptions.length > 0
      ? Math.min(selectedIndex, dropdownOptions.length - 1)
      : 0;

  const resetInput = useCallback(() => {
    setInputValue("");
    setShowDropdown(false);
    setSelectedIndex(0);
  }, []);

  const handleAddParticipant = useCallback(
    (option: Candidate) => {
      addParticipant(option);
      resetInput();
    },
    [addParticipant, resetInput],
  );

  const handleInputChange = useCallback((value: string) => {
    setInputValue(value);
    setShowDropdown(true);
    setSelectedIndex(0);
  }, []);

  return {
    inputValue,
    showDropdown,
    setShowDropdown,
    selectedIndex: activeSelectedIndex,
    setSelectedIndex,
    mappingIds,
    pendingParticipants,
    dropdownOptions,
    handleAddParticipant,
    handleInputChange,
    deleteLastParticipant,
    resetInput,
  };
}

function useEventContactEnhancement(sessionId: string) {
  const session = useSession(sessionId);
  const userId = session?.user_id;
  const sessionEvent = session ? getSessionEvent(session) : null;
  const participants = useSessionParticipants(sessionId);
  const humans = useHumans();
  const eventParticipants = useSessionEventParticipants(sessionId);
  const model = useLanguageModel("title");

  const showEnhancementButtons = Boolean(
    sessionEvent?.title?.trim() || sessionEvent?.description?.trim(),
  );

  const { isPending, mutate, variables } = useMutation({
    mutationKey: ["event-contact-enhancement", sessionId],
    mutationFn: async (humanId: string) => {
      if (!sessionEvent || !userId) {
        throw new Error("Event unavailable");
      }

      const context = buildEventContactExtractionContextFromRecords({
        sessionEvent,
        currentUserId: userId,
        participants,
        eventParticipants,
      });
      const extraction = await extractEventContacts({ model, context });
      const participant = participants.find(
        (candidate) => candidate.humanId === humanId,
      );
      const human = humans.find((candidate) => candidate.id === humanId);
      const currentUser = humans.find((candidate) => candidate.id === userId);
      const { result: applied, changes } = planExtractedContactToHuman({
        humanId,
        userId,
        human,
        currentUser,
        mappingSource: participant?.source,
        contacts: extraction.contacts,
      });
      await applyContactEnhancement({
        humanId,
        ownerUserId: userId,
        changes,
      });

      return { extraction, applied, humanId };
    },
    onSuccess: ({ extraction, applied, humanId }) => {
      const changed = applied.created + applied.updated + applied.linked;
      const toastId = `event-contact-enhancement-${humanId}`;

      if (extraction.contacts.length === 0 || !applied.matched) {
        sonnerToast.info("No contact detail found", { id: toastId });
        return;
      }

      if (changed === 0) {
        sonnerToast.info("Contact already up to date", { id: toastId });
        return;
      }

      sonnerToast.success("Contact enhanced", { id: toastId });
    },
    onError: (error) => {
      const message =
        error instanceof Error && error.message === "Language model needed"
          ? "Language model needed"
          : "Could not enhance contact";

      sonnerToast.error(message, {
        id: "event-contact-enhancement",
      });
    },
  });

  const enhanceContact = useCallback(
    (humanId: string) => {
      mutate(humanId);
    },
    [mutate],
  );

  return {
    enhanceContact,
    enhancingHumanId: isPending ? variables : undefined,
    showEnhancementButtons,
  };
}
