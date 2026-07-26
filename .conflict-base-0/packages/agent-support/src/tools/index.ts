import {
  coreTools,
  createToolRegistry,
  readUrlTool,
  toolsRequiringApproval,
} from "@hypr/agent-core";

import { executeCodeTool } from "./execute-code";
import { loopsTool } from "./loops";
import { posthogTool } from "./posthog";
import { stripeTool } from "./stripe";
import { supabaseTool } from "./supabase";

const registry = createToolRegistry([
  ...coreTools,
  executeCodeTool,
  loopsTool,
  posthogTool,
  stripeTool,
  supabaseTool,
]);

export const { tools, toolsByName, registerTool } = registry;

export {
  executeCodeTool,
  loopsTool,
  posthogTool,
  readUrlTool,
  stripeTool,
  supabaseTool,
  toolsRequiringApproval,
};
