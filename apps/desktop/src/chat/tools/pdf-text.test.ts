import { deflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";

import { extractPdfText, isPdfMaterial } from "./pdf-text";

describe("pdf text extraction", () => {
  it("recognizes PDF materials", () => {
    expect(isPdfMaterial("application/pdf", "syllabus.pdf")).toBe(true);
    expect(isPdfMaterial("text/plain", "notes.txt")).toBe(false);
  });

  it("extracts show-text operators from a FlateDecode stream", async () => {
    const bytes = buildPdf("(Office hours: Friday 2pm) Tj");

    await expect(extractPdfText(bytes)).resolves.toBe(
      "Office hours: Friday 2pm",
    );
  });

  it("extracts TJ arrays and literal escapes", async () => {
    const bytes = buildPdf("[(Week 1: ) 20 (intro\\nlab)] TJ");

    await expect(extractPdfText(bytes)).resolves.toBe("Week 1: intro lab");
  });

  it("resolves an indirect /Length object instead of treating the ref as bytes", async () => {
    const bytes = buildPdf("(Office hours: Friday 2pm) Tj", {
      indirectLength: true,
    });

    await expect(extractPdfText(bytes)).resolves.toBe(
      "Office hours: Friday 2pm",
    );
  });

  it("scans to endstream when an indirect /Length object is missing", async () => {
    const bytes = buildPdf("(Office hours: Friday 2pm) Tj", {
      lengthRef: "99 0 R",
    });

    await expect(extractPdfText(bytes)).resolves.toBe(
      "Office hours: Friday 2pm",
    );
  });

  it("returns null for non-PDF bytes", async () => {
    await expect(
      extractPdfText(Array.from(new TextEncoder().encode("hello"))),
    ).resolves.toBeNull();
  });
});

function buildPdf(
  contentStream: string,
  options: { indirectLength?: boolean; lengthRef?: string } = {},
): Uint8Array {
  const compressed = deflateSync(
    Buffer.from(`BT ${contentStream} ET`, "latin1"),
  );
  const length = options.lengthRef
    ? options.lengthRef
    : options.indirectLength
      ? "2 0 R"
      : String(compressed.length);
  const lengthObject = options.indirectLength
    ? `2 0 obj\n${compressed.length}\nendobj\n`
    : "";
  const header = `%PDF-1.1\n${lengthObject}1 0 obj\n<< /Length ${length} /Filter /FlateDecode >>\nstream\n`;
  const stream = Buffer.concat([
    Buffer.from(header, "latin1"),
    compressed,
    Buffer.from("\nendstream\nendobj\n", "latin1"),
  ]);
  return new Uint8Array(stream);
}
