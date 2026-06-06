import { useCallback, useMemo, useState } from "react";

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

export function SpeakerAssignPopover({
  segment,
  transcriptId,
  color,
  label,
}: {
  segment: Segment;
  transcriptId: string;
  color: string;
  label: string;
}) {
  const [open, setOpen] = useState(false);
  const store = main.UI.useStore(main.STORE_ID);
  const isSelf = segment.key.channel === "DirectMic";

  const sessionId = main.UI.useCell(
    "transcripts",
    transcriptId,
    "session_id",
    main.STORE_ID,
  ) as string | undefined;

  const handleAssign = useCallback(
    (humanId: string) => {
      if (!store || segment.words.length === 0) return;
      const anchorWordId = getAssignmentAnchorWordId(segment);
      if (!anchorWordId) return;
      upsertSpeakerAssignment(
        store,
        transcriptId,
        segment.key,
        humanId,
        anchorWordId,
      );
      setOpen(false);
    },
    [store, transcriptId, segment.key, segment.words],
  );

  if (isSelf) {
    return <span style={{ color }}>{label}</span>;
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn([
            "-ml-1 cursor-pointer rounded-xs px-1",
            "hover:bg-accent transition-colors",
          ])}
          style={{ color }}
        >
          {label}
        </button>
      </PopoverTrigger>
      <PopoverContent variant="app" align="start" className="w-64">
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

export type SpeakerParticipantOption = {
  id: string;
  name: string;
  email?: string;
  isSessionParticipant: boolean;
  isNew?: boolean;
};

export function buildSpeakerParticipantGroups({
  sessionParticipants,
  contacts,
  query,
}: {
  sessionParticipants: SpeakerParticipantOption[];
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

  const sessionParticipantIds = new Set(
    sessionParticipants.map((option) => option.id),
  );
  const matchingSessionParticipants = sessionParticipants.filter(matches);
  const matchingContacts = contacts
    .filter((option) => !sessionParticipantIds.has(option.id))
    .filter(matches);

  return [
    ...(matchingSessionParticipants.length > 0
      ? [
          {
            title: "Session participants",
            options: matchingSessionParticipants,
          },
        ]
      : []),
    ...(matchingContacts.length > 0
      ? [
          {
            title: "Contacts",
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
  };
}

function ParticipantList({
  sessionId,
  onSelect,
}: {
  sessionId: string | undefined;
  onSelect: (humanId: string) => void;
}) {
  const queries = main.UI.useQueries(main.STORE_ID);
  const store = main.UI.useStore(main.STORE_ID);
  const userId = main.UI.useValue("user_id", main.STORE_ID) as
    | string
    | undefined;
  const allHumanIds = main.UI.useRowIds("humans", main.STORE_ID) as string[];

  const mappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId ?? "",
    main.STORE_ID,
  ) as string[];

  const [query, setQuery] = useState("");

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

  const groups = useMemo(
    () =>
      buildSpeakerParticipantGroups({
        sessionParticipants: participants,
        contacts,
        query,
      }),
    [contacts, participants, query],
  );

  const createOption = useMemo(
    () =>
      buildCreateSpeakerParticipantOption({
        query,
        existingOptions: [...participants, ...contacts],
      }),
    [contacts, participants, query],
  );

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
    (name: string) => {
      if (!store || !userId) {
        return null;
      }

      const humanId = crypto.randomUUID();
      store.setRow("humans", humanId, {
        user_id: userId,
        created_at: new Date().toISOString(),
        name,
        email: "",
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

  const handleSelect = useCallback(
    (option: SpeakerParticipantOption) => {
      const humanId = option.isNew ? createHuman(option.name) : option.id;
      if (!humanId) {
        return;
      }

      linkHumanToSession(humanId);
      onSelect(humanId);
    },
    [createHuman, linkHumanToSession, onSelect],
  );

  return (
    <AppFloatingPanel className="overflow-hidden">
      <div className="border-border border-b p-2">
        <input
          autoFocus
          type="search"
          className={cn([
            "border-border bg-card h-8 w-full rounded-md border px-2 text-sm outline-hidden",
            "placeholder:text-muted-foreground focus:border-border",
          ])}
          placeholder="Search contacts"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      <div className="max-h-56 overflow-auto py-1">
        {createOption && (
          <ParticipantOptionButton
            option={createOption}
            onSelect={handleSelect}
          />
        )}

        {groups.map((group) => (
          <div key={group.title}>
            <div className="text-muted-foreground px-3 pt-2 pb-1 text-[11px] font-medium uppercase">
              {group.title}
            </div>
            {group.options.map((option) => (
              <ParticipantOptionButton
                key={option.id}
                option={option}
                onSelect={handleSelect}
              />
            ))}
          </div>
        ))}

        {!createOption && groups.length === 0 && (
          <p className="text-muted-foreground px-3 py-2 text-xs">
            {query.trim() ? "No matching contacts" : "No contacts"}
          </p>
        )}
      </div>
    </AppFloatingPanel>
  );
}

function ParticipantOptionButton({
  option,
  onSelect,
}: {
  option: SpeakerParticipantOption;
  onSelect: (option: SpeakerParticipantOption) => void;
}) {
  return (
    <button
      type="button"
      className={cn([
        "w-full px-3 py-1.5 text-left text-sm",
        "hover:bg-accent",
      ])}
      onClick={() => onSelect(option)}
    >
      <span className="block truncate">
        {option.isNew ? `Add "${option.name}"` : option.name}
      </span>
      {!option.isNew && option.email && (
        <span className="text-muted-foreground block truncate text-xs">
          {option.email}
        </span>
      )}
    </button>
  );
}
