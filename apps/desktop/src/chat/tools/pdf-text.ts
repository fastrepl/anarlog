const MAX_STREAMS = 64;
const MAX_INFLATE_BYTES = 2_000_000;

export function isPdfMaterial(contentType: string, filename: string): boolean {
  return (
    contentType === "application/pdf" ||
    contentType === "application/x-pdf" ||
    /\.pdf$/i.test(filename)
  );
}

export async function extractPdfText(
  bytes: Uint8Array | number[],
): Promise<string | null> {
  const data = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
  if (data.length < 5) {
    return null;
  }

  const header = latin1Decode(data.subarray(0, 5));
  if (header !== "%PDF-") {
    return null;
  }

  const chunks: string[] = [];

  for (const stream of collectPdfStreams(data).slice(0, MAX_STREAMS)) {
    const content = await decodePdfStream(stream);
    if (!content) {
      continue;
    }
    const text = extractPdfOperators(content);
    if (text) {
      chunks.push(text);
    }
  }

  const joined = chunks
    .join("\n")
    .replace(/[ \t]+\n/g, "\n")
    .trim();
  return joined || null;
}

type PdfStream = {
  dictionary: string;
  data: Uint8Array;
};

function collectPdfStreams(bytes: Uint8Array): PdfStream[] {
  const source = latin1Decode(bytes);
  const streams: PdfStream[] = [];
  const pattern = /stream\r?\n/g;
  let match: RegExpExecArray | null = pattern.exec(source);
  while (match) {
    const dictionary = source.slice(
      Math.max(0, match.index - 512),
      match.index,
    );
    const dataStart = match.index + match[0].length;
    const declaredLength = pdfStreamByteLength(dictionary, source);
    const dataEnd =
      declaredLength !== null
        ? Math.min(dataStart + declaredLength, bytes.length)
        : source.indexOf("endstream", dataStart);
    if (dataEnd < dataStart) {
      match = pattern.exec(source);
      continue;
    }

    streams.push({
      dictionary,
      data: bytes.subarray(dataStart, dataEnd),
    });
    match = pattern.exec(source);
  }
  return streams;
}

function pdfStreamByteLength(
  dictionary: string,
  source: string,
): number | null {
  const indirect = dictionary.match(/\/Length\s+(\d+)\s+(\d+)\s+R\b/);
  if (indirect) {
    const objectId = indirect[1] ?? "";
    const generation = indirect[2] ?? "0";
    const object = source.match(
      new RegExp(
        `(?:^|\\s)${objectId}\\s+${generation}\\s+obj\\s+(\\d+)\\s+endobj`,
      ),
    );
    const resolved = Number(object?.[1]);
    return Number.isFinite(resolved) ? resolved : null;
  }

  const direct = dictionary.match(/\/Length\s+(\d+)(?!\s+\d+\s+R\b)/);
  const declared = Number(direct?.[1]);
  return Number.isFinite(declared) ? declared : null;
}

async function decodePdfStream(stream: PdfStream): Promise<string | null> {
  const filtered = /\/Filter\b/.test(stream.dictionary);
  const flate = /\/FlateDecode\b/.test(stream.dictionary);
  if (flate) {
    const inflated = await inflateZlib(trimPdfStream(stream.data));
    if (!inflated) {
      return null;
    }
    return latin1Decode(inflated);
  }

  if (filtered) {
    return null;
  }

  return latin1Decode(trimPdfStream(stream.data));
}

function trimPdfStream(data: Uint8Array): Uint8Array {
  if (
    data.length >= 2 &&
    data[data.length - 2] === 0x0d &&
    data[data.length - 1] === 0x0a
  ) {
    return data.subarray(0, data.length - 2);
  }
  if (
    data.length >= 1 &&
    (data[data.length - 1] === 0x0a || data[data.length - 1] === 0x0d)
  ) {
    return data.subarray(0, data.length - 1);
  }
  return data;
}

async function inflateZlib(data: Uint8Array): Promise<Uint8Array | null> {
  const copy = Uint8Array.from(data);
  for (const format of ["deflate", "deflate-raw"] as const) {
    try {
      const decompressor = new DecompressionStream(format);
      const writer = decompressor.writable.getWriter();
      await writer.write(copy);
      await writer.close();
      const inflated = new Uint8Array(
        await new Response(decompressor.readable).arrayBuffer(),
      );
      if (inflated.byteLength > MAX_INFLATE_BYTES) {
        return inflated.subarray(0, MAX_INFLATE_BYTES);
      }
      return inflated;
    } catch {
      continue;
    }
  }
  return null;
}

function extractPdfOperators(content: string): string {
  const parts: string[] = [];
  let index = 0;

  while (index < content.length) {
    const char = content[index];
    if (char === "(") {
      const parsed = readPdfLiteral(content, index);
      if (!parsed) {
        index += 1;
        continue;
      }
      index = parsed.end;
      if (isPdfShowOperator(content, index)) {
        parts.push(parsed.value);
      }
      continue;
    }

    if (char === "<" && content[index + 1] !== "<") {
      const parsed = readPdfHex(content, index);
      if (!parsed) {
        index += 1;
        continue;
      }
      index = parsed.end;
      if (isPdfShowOperator(content, index)) {
        parts.push(parsed.value);
      }
      continue;
    }

    if (char === "[") {
      const parsed = readPdfTjArray(content, index);
      if (parsed) {
        index = parsed.end;
        if (isPdfShowOperator(content, index)) {
          parts.push(parsed.value);
        }
        continue;
      }
    }

    index += 1;
  }

  return parts.join(" ").replace(/\s+/g, " ").trim();
}

function isPdfShowOperator(content: string, index: number): boolean {
  const match = content.slice(index).match(/^\s*(\'|\"|Tj|TJ)/);
  return match !== null;
}

function readPdfLiteral(
  content: string,
  start: number,
): { value: string; end: number } | null {
  if (content[start] !== "(") {
    return null;
  }

  let depth = 0;
  let escaped = false;
  let value = "";

  for (let index = start; index < content.length; index += 1) {
    const char = content[index] ?? "";
    if (escaped) {
      value += unescapePdfChar(char);
      escaped = false;
      continue;
    }
    if (char === "\\") {
      escaped = true;
      continue;
    }
    if (char === "(") {
      depth += 1;
      if (depth > 1) {
        value += char;
      }
      continue;
    }
    if (char === ")") {
      depth -= 1;
      if (depth === 0) {
        return { value, end: index + 1 };
      }
      value += char;
      continue;
    }
    value += char;
  }

  return null;
}

function unescapePdfChar(char: string): string {
  switch (char) {
    case "n":
      return "\n";
    case "r":
      return "\r";
    case "t":
      return "\t";
    case "b":
      return "\b";
    case "f":
      return "\f";
    default:
      return char;
  }
}

function readPdfHex(
  content: string,
  start: number,
): { value: string; end: number } | null {
  if (content[start] !== "<") {
    return null;
  }

  const end = content.indexOf(">", start + 1);
  if (end < 0) {
    return null;
  }

  const hex = (content.slice(start + 1, end).replace(/\s+/g, "") + "0").slice(
    0,
    content.slice(start + 1, end).replace(/\s+/g, "").length +
      (content.slice(start + 1, end).replace(/\s+/g, "").length % 2),
  );
  let value = "";
  for (let index = 0; index < hex.length; index += 2) {
    const code = Number.parseInt(hex.slice(index, index + 2), 16);
    if (Number.isFinite(code) && code >= 32 && code < 127) {
      value += String.fromCharCode(code);
    } else if (Number.isFinite(code) && code >= 128) {
      value += String.fromCharCode(code);
    }
  }
  return { value, end: end + 1 };
}

function readPdfTjArray(
  content: string,
  start: number,
): { value: string; end: number } | null {
  if (content[start] !== "[") {
    return null;
  }

  const parts: string[] = [];
  let index = start + 1;
  while (index < content.length) {
    const char = content[index];
    if (char === "]") {
      return { value: parts.join(""), end: index + 1 };
    }
    if (char === "(") {
      const parsed = readPdfLiteral(content, index);
      if (!parsed) {
        return null;
      }
      parts.push(parsed.value);
      index = parsed.end;
      continue;
    }
    if (char === "<" && content[index + 1] !== "<") {
      const parsed = readPdfHex(content, index);
      if (!parsed) {
        return null;
      }
      parts.push(parsed.value);
      index = parsed.end;
      continue;
    }
    index += 1;
  }

  return null;
}

function latin1Decode(bytes: Uint8Array): string {
  return new TextDecoder("latin1").decode(bytes);
}
