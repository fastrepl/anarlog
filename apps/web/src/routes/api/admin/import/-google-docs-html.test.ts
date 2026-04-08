import assert from "node:assert/strict";
import test from "node:test";

import { normalizeGoogleDocsBodyContent } from "./-google-docs-html.ts";

test("wraps consecutive Google Docs monospace paragraphs in a code block", () => {
  const html = `
    <html>
      <head>
        <style type="text/css">
          .c0{font-family:"Arial"}
          .c1{font-family:"Courier New"}
          .c2{padding-bottom:8pt}
          .c4{padding-bottom:0pt}
        </style>
      </head>
      <body>
        <p class="c2"><span class="c0">Add this to the note:</span></p>
        <p class="c4"><span class="c1">---</span></p>
        <p class="c4"><span class="c1">role: Engineering Lead</span></p>
        <p class="c4"><span class="c1">company: Acme</span></p>
        <p class="c4"><span class="c1">&nbsp; - person</span></p>
      </body>
    </html>
  `;

  const normalized = normalizeGoogleDocsBodyContent(html);

  assert.match(
    normalized,
    /<pre><code><span class="c1">---<\/span>\n<span class="c1">role: Engineering Lead<\/span>\n<span class="c1">company: Acme<\/span>\n<span class="c1">  - person<\/span><\/code><\/pre>/,
  );
  assert.doesNotMatch(normalized, /<p class="c4">/);
});

test("converts inline monospace spans into inline code without creating code blocks", () => {
  const html = `
    <html>
      <head>
        <style type="text/css">
          .c0{font-family:"Arial"}
          .c2{padding-bottom:8pt}
          .c7{font-family:"Courier New"}
        </style>
      </head>
      <body>
        <p class="c2">
          <span class="c0">The </span>
          <span class="c7">role</span>
          <span class="c0"> field should stay inline.</span>
        </p>
      </body>
    </html>
  `;

  const normalized = normalizeGoogleDocsBodyContent(html);

  assert.doesNotMatch(normalized, /<pre><code>/);
  assert.match(
    normalized,
    /<p class="c2">[\s\S]*<code>role<\/code>[\s\S]*<\/p>/,
  );
  assert.doesNotMatch(normalized, /<span class="c7">role<\/span>/);
});
