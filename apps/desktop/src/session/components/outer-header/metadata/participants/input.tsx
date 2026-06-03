import { useMutation } from "@tanstack/react-query";
import { Loader2Icon, SparklesIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import { ParticipantChip } from "./chip";
import { ParticipantDropdown } from "./dropdown";
import {
  applyExtractedContacts,
  buildEventContactExtractionContext,
  extractEventContacts,
} from "./event-contact-extraction";

import { useLanguageModel } from "~/ai/hooks";
import { useAutoCloser } from "~/shared/hooks/useAutoCloser";
import { showTransientToast } from "~/sidebar/toast/transient";
import { useSessionEvent } from "~/store/tinybase/hooks";
import * as main from "~/store/tinybase/store/main";

export function ParticipantInput({ sessionId }: { sessionId: string }) {
  const {
    inputValue,
    showDropdown,
    setShowDropdown,
    selectedIndex,
    setSelectedIndex,
    mappingIds,
    dropdownOptions,
    handleAddParticipant,
    handleInputChange,
    deleteLastParticipant,
    resetInput,
  } = useParticipantInput(sessionId);
  const { extractContacts, isExtracting, showExtractionButton } =
    useEventContactExtraction(sessionId);
  const placeholder =
    mappingIds.length > 0
      ? "Who else was in the meeting?"
      : "Who was in this meeting?";

  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useAutoCloser(() => setShowDropdown(false), {
    esc: false,
    outside: true,
  });

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
    <div className="relative" ref={containerRef}>
      <div
        className="flex min-h-[38px] w-full cursor-text flex-wrap items-center gap-2"
        onClick={() => inputRef.current?.focus()}
      >
        {mappingIds.map((mappingId) => (
          <ParticipantChip key={mappingId} mappingId={mappingId} />
        ))}

        <input
          ref={inputRef}
          type="text"
          className="min-w-[120px] flex-1 bg-transparent text-sm outline-hidden placeholder:text-neutral-400"
          placeholder={placeholder}
          value={inputValue}
          onChange={(e) => handleInputChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onFocus={() => setShowDropdown(true)}
        />

        {showExtractionButton && (
          <ExtractEventContactsButton
            isExtracting={isExtracting}
            onClick={extractContacts}
          />
        )}
      </div>

      {showDropdown && inputValue.trim() && (
        <ParticipantDropdown
          options={dropdownOptions}
          selectedIndex={selectedIndex}
          onSelect={handleSelect}
          onHover={setSelectedIndex}
        />
      )}
    </div>
  );
}

function ExtractEventContactsButton({
  isExtracting,
  onClick,
}: {
  isExtracting: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip delayDuration={0}>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label="Extract contacts from event"
          className="size-6 shrink-0 text-neutral-400 hover:text-neutral-700"
          disabled={isExtracting}
          onClick={(event) => {
            event.stopPropagation();
            onClick();
          }}
        >
          {isExtracting ? (
            <Loader2Icon className="size-3.5 animate-spin" />
          ) : (
            <SparklesIcon className="size-3.5" />
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">Extract contacts from event</TooltipContent>
    </Tooltip>
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

function useSessionParticipants(sessionId: string) {
  const queries = main.UI.useQueries(main.STORE_ID);

  const mappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId,
    main.STORE_ID,
  ) as string[];

  const existingHumanIds = useMemo(() => {
    if (!queries) {
      return new Set<string>();
    }

    const ids = new Set<string>();
    for (const mappingId of mappingIds) {
      const result = queries.getResultRow(
        main.QUERIES.sessionParticipantsWithDetails,
        mappingId,
      );
      if (result?.human_id) {
        ids.add(result.human_id as string);
      }
    }
    return ids;
  }, [mappingIds, queries]);

  return { mappingIds, existingHumanIds };
}

function useCandidateSearch(
  inputValue: string,
  existingHumanIds: Set<string>,
): Candidate[] {
  const store = main.UI.useStore(main.STORE_ID);
  const allHumanIds = main.UI.useRowIds("humans", main.STORE_ID) as string[];

  return useMemo(() => {
    const searchLower = inputValue.toLowerCase();
    return allHumanIds
      .filter((humanId: string) => !existingHumanIds.has(humanId))
      .map((humanId: string) => {
        const human = store?.getRow("humans", humanId);
        if (!human) {
          return null;
        }

        const name = (human.name || "") as string;
        const email = (human.email || "") as string;
        const phone = (human.phone || "") as string;
        const nameMatch = name.toLowerCase().includes(searchLower);
        const emailMatch = email.toLowerCase().includes(searchLower);
        const phoneMatch = phone.toLowerCase().includes(searchLower);

        if (inputValue && !nameMatch && !emailMatch && !phoneMatch) {
          return null;
        }

        return {
          id: humanId,
          name,
          email,
          phone,
          orgId: human.org_id as string | undefined,
          jobTitle: human.job_title as string | undefined,
          isNew: false,
        };
      })
      .filter((h): h is NonNullable<typeof h> => h !== null);
  }, [inputValue, allHumanIds, existingHumanIds, store]);
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

function useParticipantMutations(sessionId: string, mappingIds: string[]) {
  const store = main.UI.useStore(main.STORE_ID);
  const userId = main.UI.useValue("user_id", main.STORE_ID);

  const createHuman = useCreateHuman(userId || "");
  const linkHumanToSession = useLinkHumanToSession(userId || "", sessionId);

  const addParticipant = useCallback(
    (option: Candidate) => {
      if (!userId) {
        return;
      }

      if (option.isNew) {
        const humanId = createHuman(option.name);
        linkHumanToSession(humanId);
      } else {
        linkHumanToSession(option.id);
      }
    },
    [userId, createHuman, linkHumanToSession],
  );

  const deleteLastParticipant = useCallback(() => {
    if (mappingIds.length > 0 && store) {
      const lastMappingId = mappingIds[mappingIds.length - 1];
      store.delRow("mapping_session_participant", lastMappingId);
    }
  }, [mappingIds, store]);

  return { addParticipant, deleteLastParticipant };
}

function useParticipantInput(sessionId: string) {
  const [inputValue, setInputValue] = useState("");
  const [showDropdown, setShowDropdown] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const { mappingIds, existingHumanIds } = useSessionParticipants(sessionId);
  const candidates = useCandidateSearch(inputValue, existingHumanIds);
  const dropdownOptions = useDropdownOptions(inputValue, candidates);
  const { addParticipant, deleteLastParticipant } = useParticipantMutations(
    sessionId,
    mappingIds,
  );

  useEffect(() => {
    if (selectedIndex >= dropdownOptions.length && dropdownOptions.length > 0) {
      setSelectedIndex(dropdownOptions.length - 1);
    } else if (dropdownOptions.length === 0) {
      setSelectedIndex(0);
    }
  }, [dropdownOptions.length, selectedIndex]);

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
    selectedIndex,
    setSelectedIndex,
    mappingIds,
    dropdownOptions,
    handleAddParticipant,
    handleInputChange,
    deleteLastParticipant,
    resetInput,
  };
}

function useEventContactExtraction(sessionId: string) {
  const store = main.UI.useStore(main.STORE_ID);
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const sessionEvent = useSessionEvent(sessionId);
  const model = useLanguageModel("title");

  const showExtractionButton = Boolean(
    sessionEvent?.title?.trim() || sessionEvent?.description?.trim(),
  );

  const { mutate, isPending } = useMutation({
    mutationKey: ["event-contact-extraction", sessionId],
    mutationFn: async () => {
      if (!store || !sessionEvent) {
        throw new Error("Event unavailable");
      }

      const context = buildEventContactExtractionContext(
        store,
        sessionId,
        sessionEvent,
      );
      const extraction = await extractEventContacts({ model, context });
      const applied = applyExtractedContacts(
        store,
        sessionId,
        extraction.contacts,
        {
          userId: typeof userId === "string" ? userId : undefined,
        },
      );

      return { extraction, applied };
    },
    onSuccess: ({ extraction, applied }) => {
      const changed = applied.created + applied.updated + applied.linked;
      if (extraction.contacts.length === 0) {
        showTransientToast({
          id: "event-contact-extraction",
          description: "No contact hint found",
        });
        return;
      }

      if (changed === 0) {
        showTransientToast({
          id: "event-contact-extraction",
          description: "Contacts already up to date",
        });
        return;
      }

      showTransientToast({
        id: "event-contact-extraction",
        description: formatExtractionToastDescription(applied),
      });
    },
    onError: (error) => {
      const message =
        error instanceof Error && error.message === "Language model needed"
          ? "Language model needed"
          : "Could not extract contacts";

      showTransientToast({
        id: "event-contact-extraction",
        description: message,
        variant: "error",
      });
    },
  });

  const extractContacts = useCallback(() => {
    mutate();
  }, [mutate]);

  return {
    extractContacts,
    isExtracting: isPending,
    showExtractionButton,
  };
}

function formatExtractionToastDescription({
  created,
  updated,
  linked,
}: {
  created: number;
  updated: number;
  linked: number;
}) {
  if (created > 0 && updated === 0) {
    return created === 1 ? "Contact created" : `${created} contacts created`;
  }

  if (updated > 0 && created === 0) {
    return updated === 1 ? "Contact updated" : `${updated} contacts updated`;
  }

  if (linked > 0 && created === 0 && updated === 0) {
    return linked === 1 ? "Contact linked" : `${linked} contacts linked`;
  }

  return "Contacts updated";
}

function useLinkHumanToSession(
  userId: string,
  sessionId: string,
): (humanId: string) => void {
  const linkMapping = main.UI.useSetRowCallback(
    "mapping_session_participant",
    () => crypto.randomUUID(),
    (p: { humanId: string }) => ({
      user_id: userId,
      created_at: new Date().toISOString(),
      session_id: sessionId,
      human_id: p.humanId,
      source: "manual",
    }),
    [userId, sessionId],
    main.STORE_ID,
  );

  return useCallback(
    (humanId: string) => {
      linkMapping({ humanId });
    },
    [linkMapping],
  );
}

function useCreateHuman(userId: string): (name: string) => string {
  const createHuman = main.UI.useSetRowCallback(
    "humans",
    (p: { name: string; humanId: string }) => p.humanId,
    (p: { name: string; humanId: string }) => ({
      user_id: userId,
      created_at: new Date().toISOString(),
      name: p.name,
      email: "",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
    }),
    [userId],
    main.STORE_ID,
  );

  return useCallback(
    (name: string) => {
      const humanId = crypto.randomUUID();
      createHuman({ name, humanId });
      return humanId;
    },
    [createHuman],
  );
}
