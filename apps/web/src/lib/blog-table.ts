import {
  Children,
  cloneElement,
  createElement,
  isValidElement,
  type ReactElement,
  type ReactNode,
} from "react";

export function prepareBlogTable(children: ReactNode) {
  const sections = normalizeTableChildren(children);
  const head = sections
    .map(getElementWithChildren)
    .find((section) => section?.type === "thead");
  const headerRows = Children.toArray(head?.props.children).filter(
    (row) => getElementWithChildren(row)?.type === "tr",
  );
  const headers = headerRows.length === 1 ? getCells(headerRows[0]) : [];
  const labels = headers.map((cell) => textContent(cell.props.children).trim());
  const rows = sections.flatMap((section) => {
    const element = getElementWithChildren(section);
    return element?.type === "tbody"
      ? Children.toArray(element.props.children).filter(
          (row) => getElementWithChildren(row)?.type === "tr",
        )
      : [];
  });
  const allCells = [...headers, ...rows.flatMap(getCells)];
  const stackOnMobile =
    labels.length >= 3 &&
    labels.every(Boolean) &&
    rows.length > 0 &&
    !sections.some(
      (section) => getElementWithChildren(section)?.type === "tfoot",
    ) &&
    rows.every((row) => getCells(row).length === labels.length) &&
    allCells.every(
      (cell) =>
        (cell.props.colSpan ?? 1) === 1 && (cell.props.rowSpan ?? 1) === 1,
    );

  if (!stackOnMobile) return { children: sections, stackOnMobile };

  return {
    stackOnMobile,
    children: sections.map((section) => {
      const element = getElementWithChildren(section);
      if (!element || (element.type !== "thead" && element.type !== "tbody"))
        return section;
      const isHeader = element.type === "thead";
      return cloneElement(
        element,
        { role: "rowgroup" },
        Children.toArray(element.props.children).map((row) => {
          const rowElement = getElementWithChildren(row);
          if (!rowElement || rowElement.type !== "tr") return row;
          let column = 0;
          return cloneElement(
            rowElement,
            { role: "row" },
            Children.toArray(rowElement.props.children).map((child) => {
              const cell = getElementWithChildren(child);
              if (!cell || (cell.type !== "th" && cell.type !== "td"))
                return child;
              const label = labels[column++];
              if (isHeader)
                return cloneElement(cell, {
                  role: "columnheader",
                  scope: "col",
                });
              return cloneElement(
                cell,
                {
                  role:
                    column === 1 || cell.type === "th" ? "rowheader" : "cell",
                },
                column > 1
                  ? createElement(
                      "span",
                      { className: "blog-table-label", "aria-hidden": true },
                      label,
                    )
                  : null,
                cell.props.children,
              );
            }),
          );
        }),
      );
    }),
  };
}

function getCells(row: ReactNode): ElementWithChildren[] {
  return Children.toArray(getElementWithChildren(row)?.props.children)
    .map(getElementWithChildren)
    .filter(
      (cell): cell is ElementWithChildren =>
        cell !== null && (cell.type === "th" || cell.type === "td"),
    );
}

function textContent(node: ReactNode): string {
  return Children.toArray(node)
    .map((child) => {
      if (typeof child === "string" || typeof child === "number")
        return String(child);
      return textContent(getElementWithChildren(child)?.props.children);
    })
    .join("");
}

type ElementWithChildren = ReactElement<{
  children?: ReactNode;
  colSpan?: number;
  rowSpan?: number;
  role?: string;
  scope?: string;
}>;

function normalizeTableChildren(children: ReactNode) {
  return Children.toArray(children).map((child) => {
    const element = getElementWithChildren(child);

    if (!element) {
      return child;
    }

    if (element.type === "thead") {
      const rows = Children.toArray(element.props.children);
      return rows.length > 0 && rows.every(isBlankTableRow) ? null : child;
    }

    if (element.type !== "tbody") {
      return child;
    }

    const rows = Children.toArray(element.props.children);
    const visibleRows = rows.filter((row) => !isBlankTableRow(row));

    if (visibleRows.length === rows.length) {
      return child;
    }

    return visibleRows.length > 0
      ? cloneElement(element, undefined, visibleRows)
      : null;
  });
}

function isBlankTableRow(row: ReactNode) {
  const element = getElementWithChildren(row);

  if (!element || element.type !== "tr") {
    return false;
  }

  const cells = Children.toArray(element.props.children);
  return cells.length > 0 && cells.every(isBlankTableCell);
}

function isBlankTableCell(cell: ReactNode) {
  const element = getElementWithChildren(cell);

  if (!element || (element.type !== "td" && element.type !== "th")) {
    return false;
  }

  return isBlankNode(element.props.children);
}

function isBlankNode(node: ReactNode): boolean {
  if (node === null || node === undefined || typeof node === "boolean") {
    return true;
  }

  if (typeof node === "string" || typeof node === "number") {
    return (
      String(node)
        .replace(/\u00a0/g, " ")
        .trim() === ""
    );
  }

  const element = getElementWithChildren(node);
  if (element) {
    const { children } = element.props;

    if (
      children === null ||
      children === undefined ||
      typeof children === "boolean"
    ) {
      return false;
    }

    return isBlankNode(children);
  }

  const children = Children.toArray(node);
  return children.length === 0 || children.every(isBlankNode);
}

function getElementWithChildren(node: ReactNode): ElementWithChildren | null {
  return isValidElement<{ children?: ReactNode }>(node) ? node : null;
}
