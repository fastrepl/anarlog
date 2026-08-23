import {
  Buildings,
  CalendarBlank,
  MagnifyingGlass,
  Monitor,
  User,
} from "@phosphor-icons/react";

import type { ContextEntity, ContextEntityKind } from "./entities";

import type { TabInput } from "~/store/zustand/tabs";

export type ContextChipProps = {
  key: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  removable?: boolean;
  tab?: TabInput;
};

type EntityRenderer<E extends ContextEntity> = {
  toChip: (entity: E) => ContextChipProps | null;
};

type ExtractEntity<K extends ContextEntityKind> = Extract<
  ContextEntity,
  { kind: K }
>;

type RendererMap = {
  [K in ContextEntityKind]: EntityRenderer<ExtractEntity<K>>;
};

const renderers: RendererMap = {
  session: {
    toChip: (entity) => {
      const label = entity.title || entity.date || "Session";
      const isFromTool = entity.source === "tool";
      return {
        key: entity.key,
        icon: isFromTool ? MagnifyingGlass : CalendarBlank,
        label,
        removable: entity.removable,
        tab: { type: "sessions", id: entity.sessionId },
      };
    },
  },

  human: {
    toChip: (entity) => {
      const label = entity.name || entity.email || "Person";
      return {
        key: entity.key,
        icon: User,
        label,
        removable: entity.removable,
        tab: {
          type: "contacts",
          state: { selected: { type: "person", id: entity.humanId } },
        },
      };
    },
  },

  organization: {
    toChip: (entity) => {
      const label = entity.name || "Organization";
      return {
        key: entity.key,
        icon: Buildings,
        label,
        removable: entity.removable,
        tab: {
          type: "contacts",
          state: {
            selected: { type: "organization", id: entity.organizationId },
          },
        },
      };
    },
  },

  calendar_event: {
    toChip: (entity) => {
      const label = entity.title || "Event";
      return {
        key: entity.key,
        icon: CalendarBlank,
        label,
        removable: entity.removable,
        tab: entity.linkedSessionId
          ? { type: "sessions", id: entity.linkedSessionId }
          : { type: "calendar" },
      };
    },
  },

  account: {
    toChip: (entity) => {
      if (!entity.email && !entity.userId) return null;
      return {
        key: entity.key,
        icon: User,
        label: "Account",
        tab: { type: "settings", state: { tab: "account" } },
      };
    },
  },

  device: {
    toChip: (entity) => {
      return {
        key: entity.key,
        icon: Monitor,
        label: "Device",
        tab: { type: "settings", state: { tab: "sync" } },
      };
    },
  },
} satisfies RendererMap;

export function renderChip(entity: ContextEntity): ContextChipProps | null {
  const renderer = renderers[entity.kind] as EntityRenderer<typeof entity>;
  return renderer.toChip(entity);
}
