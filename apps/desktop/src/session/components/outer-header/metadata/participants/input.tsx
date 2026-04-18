import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { ParticipantChip } from "./chip";
import { ParticipantDropdown } from "./dropdown";

import {
  useSearchableHumans,
  useSessionParticipantHumanIds,
  useSessionParticipantMappingIds,
  useSessionParticipantMutations,
} from "~/session/hooks/participants";
import { useAutoCloser } from "~/shared/hooks/useAutoCloser";

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

type Candidate = {
  id: string;
  name: string;
  email?: string;
  orgId?: string;
  jobTitle?: string;
  isNew?: boolean;
};

function useSessionParticipants(sessionId: string) {
  const mappingIds = useSessionParticipantMappingIds(sessionId);
  const participantHumanIds = useSessionParticipantHumanIds(sessionId);

  const existingHumanIds = useMemo(() => {
    return new Set(participantHumanIds);
  }, [participantHumanIds]);

  return { mappingIds, existingHumanIds };
}

function useCandidateSearch(
  inputValue: string,
  existingHumanIds: Set<string>,
): Candidate[] {
  const humans = useSearchableHumans(inputValue, existingHumanIds);

  return useMemo(() => {
    return humans.map((human) => ({
      id: human.id,
      name: human.name,
      email: human.email,
      orgId: human.orgId,
      jobTitle: human.jobTitle,
      isNew: false,
    }));
  }, [humans]);
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
  const mutations = useSessionParticipantMutations();

  const addParticipant = useCallback(
    (option: Candidate) => {
      if (option.isNew) {
        const humanId = crypto.randomUUID();
        mutations.createHuman({
          id: humanId,
          name: option.name,
          email: "",
        });
        mutations.addParticipant({
          sessionId,
          humanId,
          source: "manual",
        });
      } else {
        mutations.addParticipant({
          sessionId,
          humanId: option.id,
          source: "manual",
        });
      }
    },
    [mutations, sessionId],
  );

  const deleteLastParticipant = useCallback(() => {
    if (mappingIds.length > 0) {
      mutations.deleteMapping(mappingIds[mappingIds.length - 1]);
    }
  }, [mappingIds, mutations]);

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
