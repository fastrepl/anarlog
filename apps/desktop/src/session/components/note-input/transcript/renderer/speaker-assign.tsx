import { useCallback, useMemo, useState } from "react";

import type { EventParticipant, SessionEvent } from "@hypr/store";
import { Checkbox } from "@hypr/ui/components/ui/checkbox";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@hypr/ui/components/ui/popover";
import { cn } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";
import type { Segment } from "~/stt/live-segment";
import { upsertSpeakerAssignment } from "~/stt/utils";

type AssignmentMode = "all" | "segment";

export function SpeakerAssignPopover({
  segment,
  transcriptId,
  color,
  label,
  className,
  onAssigned,
}: {
  segment: Segment;
  transcriptId: string;
  color: string;
  label: string;
  className?: string;
  onAssigned?: (humanId: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const store = main.UI.useStore(main.STORE_ID);

  const sessionId = main.UI.useCell(
    "transcripts",
    transcriptId,
    "session_id",
    main.STORE_ID,
  ) as string | undefined;

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    setOpen(nextOpen);
  }, []);

  const handleAssign = useCallback(
    (humanId: string, assignmentMode: AssignmentMode) => {
      if (!store || segment.words.length === 0) return;
      const anchorWordId = getAssignmentAnchorWordId(segment);
      if (!anchorWordId) return;
      upsertSpeakerAssignment(
        store,
        transcriptId,
        segment.key,
        humanId,
        anchorWordId,
        {
          mode: assignmentMode,
          wordIds: getAssignmentWordIds(segment),
        },
      );
      onAssigned?.(humanId);
      handleOpenChange(false);
    },
    [handleOpenChange, onAssigned, store, transcriptId, segment],
  );

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn([
            "-my-0.5 -ml-2 cursor-pointer rounded-full px-2 py-0.5",
            "hover:bg-accent transition-colors",
            className,
          ])}
          style={{ color }}
        >
          {label}
        </button>
      </PopoverTrigger>
      <PopoverContent variant="app" align="start" className="w-72">
        <ParticipantList sessionId={sessionId} onSelect={handleAssign} />
      </PopoverContent>
    </Popover>
  );
}

export function getAssignmentAnchorWordId(
  segment: Segment,
): string | undefined {
  const word = segment.words.find(
    (word) => typeof word.id === "string" && word.id.length > 0,
  );
  return typeof word?.id === "string" ? word.id : undefined;
}

export function getAssignmentWordIds(segment: Segment): string[] {
  return segment.words
    .map((word) => word.id)
    .filter(
      (wordId): wordId is string =>
        typeof wordId === "string" && wordId.length > 0,
    );
}

export type SpeakerParticipantOption = {
  id: string;
  name: string;
  email?: string;
  isSessionParticipant: boolean;
  isNew?: boolean;
  isCreateOption?: boolean;
};

export function buildSpeakerParticipantGroups({
  sessionParticipants,
  eventParticipants = [],
  contacts,
  query,
}: {
  sessionParticipants: SpeakerParticipantOption[];
  eventParticipants?: SpeakerParticipantOption[];
  contacts: SpeakerParticipantOption[];
  query: string;
}) {
  const normalizedQuery = query.trim().toLowerCase();
  const matches = (option: SpeakerParticipantOption) => {
    if (!normalizedQuery) {
      return true;
    }

    return [option.name, option.email ?? ""].some((value) =>
      value.toLowerCase().includes(normalizedQuery),
    );
  };

  const participantKeys = new Set<string>();
  const participantOptions = [...sessionParticipants, ...eventParticipants]
    .filter((option) => {
      const keys = getSpeakerParticipantDedupeKeys(option);
      if (keys.some((key) => participantKeys.has(key))) {
        return false;
      }

      keys.forEach((key) => participantKeys.add(key));
      return true;
    })
    .filter(matches);
  const matchingContacts = contacts
    .filter((option) =>
      getSpeakerParticipantDedupeKeys(option).every(
        (key) => !participantKeys.has(key),
      ),
    )
    .filter(matches);

  return [
    ...(participantOptions.length > 0
      ? [
          {
            title: "Participants",
            options: participantOptions,
          },
        ]
      : []),
    ...(matchingContacts.length > 0
      ? [
          {
            title: "People",
            options: matchingContacts,
          },
        ]
      : []),
  ];
}

export function buildCreateSpeakerParticipantOption({
  query,
  existingOptions,
}: {
  query: string;
  existingOptions: SpeakerParticipantOption[];
}): SpeakerParticipantOption | null {
  const name = query.trim();
  if (!name) {
    return null;
  }

  const normalizedName = name.toLowerCase();
  const alreadyExists = existingOptions.some((option) =>
    [option.name, option.email ?? ""].some(
      (value) => value.toLowerCase() === normalizedName,
    ),
  );
  if (alreadyExists) {
    return null;
  }

  return {
    id: "new",
    name,
    isSessionParticipant: false,
    isNew: true,
    isCreateOption: true,
  };
}

export function buildEventSpeakerParticipantOptions({
  eventParticipants,
  contacts,
}: {
  eventParticipants: EventParticipant[];
  contacts: SpeakerParticipantOption[];
}): SpeakerParticipantOption[] {
  const contactByEmail = new Map(
    contacts
      .filter((contact) => contact.email)
      .map((contact) => [contact.email!.toLowerCase(), contact]),
  );
  const contactByName = new Map(
    contacts.map((contact) => [contact.name.toLowerCase(), contact]),
  );

  return eventParticipants
    .map((participant, index): SpeakerParticipantOption | null => {
      const name = (participant.name ?? "").trim();
      const email = (participant.email ?? "").trim();
      if (!name && !email) {
        return null;
      }

      const contact = email
        ? contactByEmail.get(email.toLowerCase())
        : name
          ? contactByName.get(name.toLowerCase())
          : undefined;

      if (contact) {
        return {
          ...contact,
          name: name || contact.name,
          email: email || contact.email,
          isSessionParticipant: true,
        };
      }

      const pendingId = email ? `event:${email}` : `event:${name}:${index}`;

      return {
        id: pendingId,
        name: name || email,
        email: email || undefined,
        isSessionParticipant: true,
        isNew: true,
      };
    })
    .filter((option): option is SpeakerParticipantOption => option !== null);
}

function ParticipantList({
  sessionId,
  onSelect,
}: {
  sessionId: string | undefined;
  onSelect: (humanId: string, mode: AssignmentMode) => void;
}) {
  const queries = main.UI.useQueries(main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);
  const userId = main.UI.useValue("user_id", main.STORE_ID) as
    | string
    | undefined;
  const allHumanIds = main.UI.useRowIds("humans", main.STORE_ID) as string[];
  const eventsTable = main.UI.useTable("events", main.STORE_ID);
  const sessionEventJson = main.UI.useCell(
    "sessions",
    sessionId ?? "",
    "event_json",
    main.STORE_ID,
  ) as string | undefined;

  const mappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId ?? "",
    main.STORE_ID,
  ) as string[];

  const [query, setQuery] = useState("");
  const [selectedOption, setSelectedOption] =
    useState<SpeakerParticipantOption | null>(null);
  const [applyToAllMatching, setApplyToAllMatching] = useState(true);

  const participants = useMemo(() => {
    if (!queries) return [];
    return mappingIds
      .map((mappingId): SpeakerParticipantOption | null => {
        const result = queries.getResultRow(
          main.QUERIES.sessionParticipantsWithDetails,
          mappingId,
        );
        if (!result?.human_id) return null;
        const name = ((result.human_name as string | undefined) || "").trim();
        const email = ((result.human_email as string | undefined) || "").trim();
        return {
          id: result.human_id as string,
          name: name || email || "Unknown",
          email: email || undefined,
          isSessionParticipant: true,
        };
      })
      .filter((p): p is SpeakerParticipantOption => p !== null);
  }, [mappingIds, queries]);

  const participantIds = useMemo(
    () => new Set(participants.map((participant) => participant.id)),
    [participants],
  );

  const contacts = useMemo(() => {
    if (!store) return [];

    return allHumanIds
      .map((humanId): SpeakerParticipantOption | null => {
        const human = store.getRow("humans", humanId);
        if (!human) {
          return null;
        }

        const name = ((human.name as string | undefined) || "").trim();
        const email = ((human.email as string | undefined) || "").trim();
        if (!name && !email) {
          return null;
        }

        return {
          id: humanId,
          name: name || email,
          email: email || undefined,
          isSessionParticipant: false,
        };
      })
      .filter((p): p is SpeakerParticipantOption => p !== null);
  }, [allHumanIds, store]);

  const eventParticipants = useMemo(
    () =>
      buildEventSpeakerParticipantOptions({
        eventParticipants: getAttachedEventParticipants(
          eventsTable,
          sessionEventJson,
        ),
        contacts,
      }),
    [contacts, eventsTable, sessionEventJson],
  );

  const groups = useMemo(
    () =>
      buildSpeakerParticipantGroups({
        sessionParticipants: participants,
        eventParticipants,
        contacts,
        query,
      }),
    [contacts, eventParticipants, participants, query],
  );

  const createOption = useMemo(
    () =>
      buildCreateSpeakerParticipantOption({
        query,
        existingOptions: [...participants, ...eventParticipants, ...contacts],
      }),
    [contacts, eventParticipants, participants, query],
  );
  const hasPeopleGroup = groups.some((group) => group.title === "People");

  const linkHumanToSession = useCallback(
    (humanId: string) => {
      if (!store || !sessionId || !userId || participantIds.has(humanId)) {
        return;
      }

      store.setRow("mapping_session_participant", crypto.randomUUID(), {
        user_id: userId,
        session_id: sessionId,
        human_id: humanId,
        source: "manual",
      });
    },
    [participantIds, sessionId, store, userId],
  );

  const createHuman = useCallback(
    (name: string, email?: string) => {
      if (!store || !userId) {
        return null;
      }

      const humanId = crypto.randomUUID();
      store.setRow("humans", humanId, {
        user_id: userId,
        created_at: new Date().toISOString(),
        name,
        email: email ?? "",
        phone: "",
        org_id: "",
        job_title: "",
        linkedin_username: "",
        memo: "",
        pinned: false,
        pin_order: 0,
      });
      return humanId;
    },
    [store, userId],
  );

  const handleSelect = useCallback((option: SpeakerParticipantOption) => {
    setSelectedOption(option);
  }, []);

  const getCurrentHumanId = useCallback(
    (option: SpeakerParticipantOption) => {
      if (!option.isNew) {
        return option.id;
      }

      const email = option.email?.trim().toLowerCase();
      const name = option.name.trim().toLowerCase();
      const existingContact = email
        ? contacts.find(
            (contact) => contact.email?.trim().toLowerCase() === email,
          )
        : contacts.find(
            (contact) => contact.name.trim().toLowerCase() === name,
          );

      return existingContact?.id ?? createHuman(option.name, option.email);
    },
    [contacts, createHuman],
  );

  const handleConfirm = useCallback(() => {
    if (!selectedOption) {
      return;
    }

    const mode: AssignmentMode = applyToAllMatching ? "all" : "segment";
    const humanId = getCurrentHumanId(selectedOption);
    if (!humanId) {
      return;
    }

    linkHumanToSession(humanId);
    onSelect(humanId, mode);
  }, [
    applyToAllMatching,
    getCurrentHumanId,
    linkHumanToSession,
    onSelect,
    selectedOption,
  ]);

  return (
    <AppFloatingPanel className="flex flex-col gap-2 p-2">
      <div className="border-app-floating-border bg-app-floating-panel overflow-hidden rounded-2xl border">
        <div className="p-2">
          <input
            autoFocus
            type="search"
            className={cn([
              "border-border h-8 w-full rounded-md border bg-transparent px-2 text-sm outline-hidden",
              "placeholder:text-muted-foreground focus:border-border",
            ])}
            placeholder="Search people"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedOption(null);
            }}
          />
        </div>
        <div className="max-h-60 overflow-auto py-1">
          {groups.map((group) => (
            <div key={group.title}>
              <div className="text-muted-foreground px-3 pt-2 pb-1 text-[11px] font-medium uppercase">
                {group.title}
              </div>
              {group.options.map((option) => (
                <ParticipantOptionButton
                  key={option.id}
                  option={option}
                  selected={selectedOption === option}
                  onSelect={handleSelect}
                />
              ))}
            </div>
          ))}

          {createOption && (
            <div>
              {!hasPeopleGroup && (
                <div className="text-muted-foreground px-3 pt-2 pb-1 text-[11px] font-medium uppercase">
                  People
                </div>
              )}
              <ParticipantOptionButton
                option={createOption}
                selected={selectedOption === createOption}
                onSelect={handleSelect}
              />
            </div>
          )}

          {!createOption && groups.length === 0 && (
            <p className="text-muted-foreground px-3 py-2 text-xs">
              {query.trim() ? "No matching people" : "No people"}
            </p>
          )}
        </div>
      </div>
      <div className="flex items-center gap-3">
        <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-2">
          <Checkbox
            checked={applyToAllMatching}
            className={cn([
              "border-white bg-white text-black",
              "data-[state=checked]:border-white data-[state=checked]:bg-white data-[state=checked]:text-black",
            ])}
            onCheckedChange={(value) => setApplyToAllMatching(value === true)}
          />
          <span className="text-muted-foreground truncate text-xs">
            Apply to all matching segments
          </span>
        </label>
        <button
          type="button"
          className={cn([
            "h-8 rounded-full bg-white px-3 text-xs font-medium text-black",
            "hover:bg-white/90",
            "disabled:pointer-events-none disabled:opacity-50",
          ])}
          disabled={!selectedOption}
          onClick={handleConfirm}
        >
          Confirm
        </button>
      </div>
    </AppFloatingPanel>
  );
}

function getSpeakerParticipantDedupeKeys(
  option: SpeakerParticipantOption,
): string[] {
  return [
    `id:${option.id}`,
    option.email ? `email:${option.email.toLowerCase()}` : null,
  ].filter((key): key is string => key !== null);
}

function getAttachedEventParticipants(
  eventsTable: Record<string, Record<string, unknown>> | undefined,
  sessionEventJson: string | undefined,
): EventParticipant[] {
  if (!eventsTable || !sessionEventJson) {
    return [];
  }

  let sessionEvent: SessionEvent;
  try {
    sessionEvent = JSON.parse(sessionEventJson) as SessionEvent;
  } catch {
    return [];
  }

  if (!sessionEvent.tracking_id) {
    return [];
  }

  for (const event of Object.values(eventsTable)) {
    if (
      event.tracking_id_event !== sessionEvent.tracking_id ||
      event.calendar_id !== sessionEvent.calendar_id
    ) {
      continue;
    }

    const participantsJson = event.participants_json;
    if (typeof participantsJson !== "string") {
      return [];
    }

    try {
      const parsed = JSON.parse(participantsJson);
      return Array.isArray(parsed) ? (parsed as EventParticipant[]) : [];
    } catch {
      return [];
    }
  }

  return [];
}

function ParticipantOptionButton({
  option,
  selected,
  onSelect,
}: {
  option: SpeakerParticipantOption;
  selected: boolean;
  onSelect: (option: SpeakerParticipantOption) => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      className={cn([
        "w-full px-3 py-1.5 text-left text-sm",
        selected ? "bg-accent text-accent-foreground" : "hover:bg-accent",
      ])}
      onClick={() => onSelect(option)}
    >
      <span className="block truncate">
        {option.isCreateOption ? `Add "${option.name}"` : option.name}
      </span>
      {option.email && (
        <span className="text-muted-foreground block truncate text-xs">
          {option.email}
        </span>
      )}
    </button>
  );
}
