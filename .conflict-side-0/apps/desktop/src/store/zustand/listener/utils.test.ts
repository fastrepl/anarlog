import { describe, expect, test } from "vitest";

import { fixSpacingForWords } from "./utils";

describe("fixSpacingForWords", () => {
  const testCases = [
    {
      transcript: "Hello",
      input: ["Hello"],
      output: [" Hello"],
    },
    {
      transcript: "Yes. Because we",
      input: ["Yes.", "Because", "we"],
      output: [" Yes.", " Because", " we"],
    },
    {
      transcript: "shouldn't",
      input: ["shouldn", "'t"],
      output: [" shouldn", "'t"],
    },
    {
      transcript: "Yes. Because we shouldn't be false.",
      input: ["Yes.", "Because", "we", "shouldn", "'t", "be", "false."],
      output: [" Yes.", " Because", " we", " shouldn", "'t", " be", " false."],
    },
  ];

  test.each(testCases)(
    "transcript: $transcript",
    ({ transcript, input, output }) => {
      expect(output.join("")).toEqual(` ${transcript}`);

      const actual = fixSpacingForWords(input, transcript);
      expect(actual).toEqual(output);
    },
  );
});
