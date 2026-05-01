import type { StructuredToolInterface } from "@langchain/core/tools";

import { executeCodeTool } from "./execute-code";
import { readUrlTool } from "./read-url";

export const coreTools: readonly StructuredToolInterface[] = [readUrlTool];

export const toolsRequiringApproval = new Set(["executeCode"]);

export { executeCodeTool, readUrlTool };
