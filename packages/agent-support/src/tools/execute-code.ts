import {
  executeCodeTool as coreExecuteCodeTool,
  formatExecutionResult,
  setExecuteCodeFunction,
} from "@anlg/agent-core";

import { executeCode } from "../modal/execute";

setExecuteCodeFunction(executeCode);

export { coreExecuteCodeTool as executeCodeTool, formatExecutionResult };
