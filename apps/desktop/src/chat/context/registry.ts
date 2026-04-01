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
  tooltip: string;
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
        tooltip: entity.title || "Session",
        removable: entity.removable,
        entityKind: "session",
        entityId: entity.sessionId,
      };
    },
  },

  human: {
    toChip: (entity) => {
      const label = entity.name || entity.email || "Person";
      const tooltip = [entity.name, entity.email, entity.organizationName]
        .filter(Boolean)
        .join(" • ");
      return {
        key: entity.key,
        icon: UserIcon,
        label,
        tooltip: tooltip || label,
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
        tooltip: label,
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
        tooltip: entity.email || "Account",
      };
    },
  },

  device: {
    toChip: (entity) => {
      return {
        key: entity.key,
        icon: MonitorIcon,
        label: "Device",
        tooltip: "Device",
      };
    },
  },
} satisfies RendererMap;

export function renderChip(entity: ContextEntity): ContextChipProps | null {
  const renderer = renderers[entity.kind] as EntityRenderer<typeof entity>;
  return renderer.toChip(entity);
}
