import type { StructuredToolInterface } from "@langchain/core/tools";

export interface ToolRegistry {
  tools: StructuredToolInterface[];
  toolsByName: Record<string, StructuredToolInterface>;
  registerTool: (tool: StructuredToolInterface) => void;
}

export function createToolRegistry(
  initialTools: StructuredToolInterface[],
): ToolRegistry {
  const tools: StructuredToolInterface[] = [...initialTools];
  const toolsByName: Record<string, StructuredToolInterface> =
    Object.fromEntries(tools.map((t) => [t.name, t]));

  function registerTool(tool: StructuredToolInterface): void {
    tools.push(tool);
    toolsByName[tool.name] = tool;
  }

  return { tools, toolsByName, registerTool };
}
