import { describe, expect, test } from "vitest";
import { z } from "zod";

import { jsonObject } from "./shared";

describe("jsonObject", () => {
  test("with it", () => {
    const a = z.object({
      field_1: z.string(),
    });
    const b = z.object({
      field_3: z.number(),
      field_2: jsonObject(a),
    });

    const raw = {
      field_3: 1,
      field_2: {
        field_1: "test",
      },
    };

    const serialized_1 = JSON.stringify(
      b.parse({
        field_3: 1,
        field_2: JSON.stringify(
          a.parse({
            field_1: "test",
          }),
        ),
      }),
    );

    const serialized_2 = JSON.stringify(
      b.parse({
        field_3: 1,
        field_2: {
          field_1: "test",
        },
      }),
    );

    const result_1 = b.safeParse(JSON.parse(serialized_1));
    expect(result_1.success).toBe(true);
    expect(result_1.data).toEqual(raw);

    const result_2 = b.safeParse(JSON.parse(serialized_2));
    expect(result_2.success).toBe(true);
    expect(result_2.data).toEqual(raw);
  });

  test("without it", () => {
    const a = z.object({
      field_1: z.string(),
    });
    const b = z.object({
      field_3: z.number(),
      field_2: a,
    });

    const raw = {
      field_3: 1,
      field_2: {
        field_1: "test",
      },
    };

    const serialized_1 = JSON.stringify({
      field_3: 1,
      field_2: JSON.stringify(
        a.parse({
          field_1: "test",
        }),
      ),
    });
    const result_1 = b.safeParse(JSON.parse(serialized_1));
    expect(result_1.success).toBe(false);

    const serialized_2 = JSON.stringify(b.parse(raw));

    const result_2 = b.safeParse(JSON.parse(serialized_2));
    expect(result_2.success).toBe(true);
    expect(result_2.data).toEqual(raw);
  });
});
