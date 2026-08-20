import type { LanguageModelMiddleware } from "ai";

// Frontier chat/reasoning models reject an explicit temperature (including 0).
// Cloud providers already pick a supported default when the field is omitted.
export const omitTemperatureMiddleware: LanguageModelMiddleware = {
  specificationVersion: "v3",
  transformParams: async ({ params }) => {
    const next = { ...params };
    delete next.temperature;
    return next;
  },
};
