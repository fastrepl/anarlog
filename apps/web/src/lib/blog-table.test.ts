import assert from "node:assert/strict";
import test from "node:test";
import { createElement as h, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { prepareBlogTable } from "./blog-table.ts";

function render(children: ReactNode) {
  const table = prepareBlogTable(children);
  return renderToStaticMarkup(
    h(
      "table",
      { role: "table", "data-mobile-stack": table.stackOnMobile || undefined },
      table.children,
    ),
  );
}

const head = h(
  "thead",
  { key: "head" },
  h(
    "tr",
    null,
    h("th", null, "Decision"),
    h("th", null, h("strong", null, "Meetily")),
    h("th", null, "Granola"),
  ),
);

test("mobile rows retain headers, links, and text while removing blank rows", () => {
  const html = render([
    head,
    h(
      "tbody",
      { key: "body" },
      h(
        "tr",
        null,
        h("td", null, " "),
        h("td", null, "\u00a0"),
        h("td", null, ""),
      ),
      h(
        "tr",
        null,
        h("td", null, "Transcription"),
        h(
          "td",
          null,
          h("a", { href: "https://docs.meetily.ai/" }, "Local engines"),
        ),
        h("td", null, "Managed services"),
      ),
    ),
  ]);
  assert.match(html, /data-mobile-stack="true"/);
  assert.equal((html.match(/role="row"/g) ?? []).length, 2);
  assert.equal(
    (html.match(/role="columnheader" scope="col"/g) ?? []).length,
    3,
  );
  assert.equal((html.match(/role="rowheader"/g) ?? []).length, 1);
  assert.equal((html.match(/role="cell"/g) ?? []).length, 2);
  assert.deepEqual(
    [
      ...html.matchAll(
        /class="blog-table-label" aria-hidden="true">([^<]+)<\/span>/g,
      ),
    ].map((match) => match[1]),
    ["Meetily", "Granola"],
  );
  assert.match(
    html,
    /<a href="https:\/\/docs.meetily.ai\/">Local engines<\/a>/,
  );
  assert.match(html, /Managed services/);
});

test("two-column and complex tables keep their native layout without guessed labels", () => {
  const row = h(
    "tr",
    null,
    h("td", null, "A"),
    h("td", null, "B"),
    h("td", null, "C"),
  );
  for (const children of [
    [
      h(
        "thead",
        { key: "head" },
        h("tr", null, h("th", null, "Tool"), h("th", null, "Price")),
      ),
      h(
        "tbody",
        { key: "body" },
        h("tr", null, h("td", null, "A"), h("td", null, "$0")),
      ),
    ],
    [h("tbody", { key: "body" }, row)],
    [
      head,
      h(
        "tbody",
        { key: "body" },
        h("tr", null, h("td", { colSpan: 3 }, "Spanning content")),
      ),
    ],
    [
      head,
      h(
        "tbody",
        { key: "body" },
        h(
          "tr",
          null,
          h("td", { rowSpan: 2 }, "A"),
          h("td", null, "B"),
          h("td", null, "C"),
        ),
      ),
    ],
    [head, h("tbody", { key: "body" }, row), h("tfoot", { key: "foot" }, row)],
  ]) {
    const html = render(children);
    assert.doesNotMatch(html, /data-mobile-stack|blog-table-label/);
    assert.match(html, /<tbody>/);
    assert.match(html, />A<|>Spanning content</);
  }
});
