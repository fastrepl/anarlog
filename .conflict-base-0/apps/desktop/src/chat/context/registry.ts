import {
  Building2Icon,
  CalendarIcon,
  MonitorIcon,
  SearchIcon,
  UserIcon,
} from "lucide-react";

import type { ContextEntity, ContextEntityKind } from "./entities";

export type ContextChipProps = {
  key: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  removable?: boolean;
  entityKind?: ContextEntityKind;
  entityId?: string;
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
        icon: isFromTool ? SearchIcon : CalendarIcon,
        label,
        removable: entity.removable,
        entityKind: "session",
        entityId: entity.sessionId,
      };
    },
  },

  human: {
    toChip: (entity) => {
      const label = entity.name || entity.email || "Person";
      return {
        key: entity.key,
        icon: UserIcon,
        label,
        removable: entity.removable,
        entityKind: "human",
        entityId: entity.humanId,
      };
    },
  },

  organization: {
    toChip: (entity) => {
      const label = entity.name || "Organization";
      return {
        key: entity.key,
        icon: Building2Icon,
        label,
        removable: entity.removable,
        entityKind: "organization",
        entityId: entity.organizationId,
      };
    },
  },

  account: {
    toChip: (entity) => {
      if (!entity.email && !entity.userId) return null;
      return {
        key: entity.key,
        icon: UserIcon,
        label: "Account",
      };
    },
  },

  device: {
    toChip: (entity) => {
      return {
        key: entity.key,
        icon: MonitorIcon,
        label: "Device",
      };
    },
  },
} satisfies RendererMap;

export function renderChip(entity: ContextEntity): ContextChipProps | null {
  const renderer = renderers[entity.kind] as EntityRenderer<typeof entity>;
  return renderer.toChip(entity);
}
