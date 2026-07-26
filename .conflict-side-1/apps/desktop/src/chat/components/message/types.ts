import type { UIMessagePart } from "ai";
import type { ReactNode } from "react";

import type { ToolPartType, Tools } from "~/chat/tools";

export type Part = UIMessagePart<{}, Tools>;
export type ToolRenderer<T extends ToolPartType = ToolPartType> = ({
  part,
}: {
  part: Extract<Part, { type: T }>;
}) => ReactNode;
