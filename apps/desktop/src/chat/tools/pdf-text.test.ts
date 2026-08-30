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

  it("returns null for non-PDF bytes", async () => {
    await expect(
      extractPdfText(Array.from(new TextEncoder().encode("hello"))),
    ).resolves.toBeNull();
  });
});

function buildPdf(contentStream: string): Uint8Array {
  const compressed = deflateSync(
    Buffer.from(`BT ${contentStream} ET`, "latin1"),
  );
  const stream = Buffer.concat([
    Buffer.from(
      `%PDF-1.1\n1 0 obj\n<< /Length ${compressed.length} /Filter /FlateDecode >>\nstream\n`,
      "latin1",
    ),
    compressed,
    Buffer.from("\nendstream\nendobj\n", "latin1"),
  ]);
  return new Uint8Array(stream);
}
