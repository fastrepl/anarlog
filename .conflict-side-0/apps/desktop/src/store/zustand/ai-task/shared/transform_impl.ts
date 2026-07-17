import type { TextStreamPart, ToolSet } from "ai";

import type { StreamTransform } from "./transform_infra";

export function addMarkdownSectionSeparators<
  TOOLS extends ToolSet = ToolSet,
>(): StreamTransform<TOOLS> {
  return () => {
    let consecutiveNewlines = 0;

    return new TransformStream<TextStreamPart<TOOLS>, TextStreamPart<TOOLS>>({
      transform(chunk, controller) {
        if (chunk.type !== "text-delta") {
          controller.enqueue(chunk);
          return;
        }

        let transformedText = "";

        for (const char of chunk.text) {
          if (char === "\n") {
            consecutiveNewlines += 1;
            transformedText += char;
            continue;
          }

          if (char === "#" && consecutiveNewlines >= 2) {
            transformedText += "<p></p>\n\n";
          }

          consecutiveNewlines = 0;
          transformedText += char;
        }

        controller.enqueue({
          ...chunk,
          text: transformedText,
        });
      },
    });
  };
}

export function normalizeBulletPoints<
  TOOLS extends ToolSet = ToolSet,
>(): StreamTransform<TOOLS> {
  return () => {
    let lineStart = true;
    let leadingWhitespace = "";

    return new TransformStream<TextStreamPart<TOOLS>, TextStreamPart<TOOLS>>({
      transform(chunk, controller) {
        if (chunk.type !== "text-delta") {
          controller.enqueue(chunk);
          return;
        }

        let transformedText = "";
        let i = 0;

        while (i < chunk.text.length) {
          const char = chunk.text[i];

          if (char === "\n") {
            transformedText += char;
            lineStart = true;
            leadingWhitespace = "";
            i++;
            continue;
          }

          if (lineStart) {
            if (char === " " || char === "\t") {
              leadingWhitespace += char;
              transformedText += char;
              i++;
              continue;
            }

            if (char === "•" && chunk.text[i + 1] === " ") {
              transformedText += "-";
              lineStart = false;
              leadingWhitespace = "";
              i++;
              continue;
            }

            lineStart = false;
            leadingWhitespace = "";
          }

          transformedText += char;
          i++;
        }

        controller.enqueue({
          ...chunk,
          text: transformedText,
        });
      },
    });
  };
}
