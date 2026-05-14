import { StandardTabWrapper } from "~/shared/main";

export function SessionSurface({
  header,
  title,
  children,
  afterBorder,
  afterBorderExpanded,
  afterBorderResizable,
  bottomBorderHandle,
  floatingButton,
  mergeAfterBorder,
}: {
  header?: React.ReactNode;
  title?: React.ReactNode;
  children: React.ReactNode;
  afterBorder?: React.ReactNode;
  afterBorderExpanded?: boolean;
  afterBorderResizable?: boolean;
  bottomBorderHandle?: React.ReactNode;
  floatingButton?: React.ReactNode;
  mergeAfterBorder?: boolean;
}) {
  return (
    <StandardTabWrapper
      afterBorder={afterBorder}
      afterBorderExpanded={afterBorderExpanded}
      afterBorderResizable={afterBorderResizable}
      bottomBorderHandle={bottomBorderHandle}
      floatingButton={floatingButton}
      mergeAfterBorder={mergeAfterBorder}
    >
      <div className="flex h-full flex-col">
        {header ? <div className="pr-1 pl-2">{header}</div> : null}
        {title ? <div className="mt-2 shrink-0 px-3">{title}</div> : null}
        <div className="mt-2 min-h-0 flex-1 px-2">{children}</div>
      </div>
    </StandardTabWrapper>
  );
}
