import type { ReactNode } from "react";

import {
  ToolCard,
  ToolCardApproval,
  ToolCardFooters,
  ToolCardHeader,
  useMcpOutput,
  useToolApproval,
  useToolState,
} from "./shared";

type ToolPart = {
  state: string;
  output?: unknown;
  // deno-lint-ignore no-explicit-any
  input?: Record<string, any>;
  errorText?: unknown;
};

// deno-lint-ignore no-explicit-any
type Input = Record<string, any> | undefined;

type ToolConfig<TParsed> = {
  icon: ReactNode;
  approval?: boolean;
  parseFn: (output: unknown) => TParsed | null;
  label: (ctx: {
    running: boolean;
    failed: boolean;
    awaitingApproval: boolean;
    parsed: TParsed | null;
    input: Input;
  }) => string;
  isDone?: (parsed: TParsed | null) => boolean;
  renderBody?: (input: Input, parsed: TParsed | null) => ReactNode;
  renderSuccess?: (parsed: TParsed) => ReactNode;
  renderFooter?: (ctx: {
    failed: boolean;
    errorText: unknown;
    rawText: string | null;
    parsed: TParsed | null;
  }) => ReactNode;
};

export function defineTool<TParsed>(config: ToolConfig<TParsed>) {
  return ({ part }: { part: ToolPart }) => {
    const { running, failed, done } = useToolState(part);
    const { parsed, rawText } = useMcpOutput(done, part.output, config.parseFn);
    const awaitingApproval =
      useToolApproval(running) && (config.approval ?? false);
    const headerDone = config.isDone ? config.isDone(parsed) : !!parsed;

    return (
      <ToolCard failed={failed}>
        <ToolCardHeader
          icon={config.icon}
          running={running}
          awaitingApproval={awaitingApproval}
          failed={failed}
          done={headerDone}
          label={config.label({
            running,
            failed,
            awaitingApproval,
            parsed,
            input: part.input,
          })}
        />
        {config.renderBody?.(part.input, parsed)}
        {awaitingApproval ? <ToolCardApproval /> : null}
        {config.renderFooter ? (
          config.renderFooter({
            failed,
            errorText: part.errorText,
            rawText,
            parsed,
          })
        ) : (
          <ToolCardFooters
            failed={failed}
            errorText={part.errorText}
            rawText={rawText}
          >
            {parsed ? config.renderSuccess?.(parsed) : null}
          </ToolCardFooters>
        )}
      </ToolCard>
    );
  };
}
