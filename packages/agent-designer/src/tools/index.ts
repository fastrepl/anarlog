import {
  createToolRegistry,
  readUrlTool,
  toolsRequiringApproval,
} from "@anlg/agent-core";

import { magicPatternsTool } from "./magic-patterns";

const registry = createToolRegistry([readUrlTool, magicPatternsTool]);

export const { tools, toolsByName, registerTool } = registry;

export { magicPatternsTool, readUrlTool, toolsRequiringApproval };
