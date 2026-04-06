import { platforms } from "./platforms";
import { templates } from "./templates";

export type BannerState = {
  layoutId: string;
  platformId: string;
  values: Record<string, string>;
  fontSizes: Record<string, number>;
};

export type BannerAction =
  | { type: "SET_LAYOUT"; layoutId: string }
  | { type: "SET_PLATFORM"; platformId: string }
  | { type: "SET_FIELD"; key: string; value: string }
  | { type: "SET_FONT_SIZE"; key: string; size: number };

export function bannerReducer(
  state: BannerState,
  action: BannerAction,
): BannerState {
  switch (action.type) {
    case "SET_LAYOUT": {
      const template = templates.find((t) => t.id === action.layoutId);
      if (!template) return state;
      const values: Record<string, string> = {};
      for (const field of template.fields) {
        values[field.key] = field.defaultValue;
      }
      return { ...state, layoutId: action.layoutId, values, fontSizes: {} };
    }
    case "SET_PLATFORM":
      return { ...state, platformId: action.platformId };
    case "SET_FIELD":
      return {
        ...state,
        values: { ...state.values, [action.key]: action.value },
      };
    case "SET_FONT_SIZE":
      return {
        ...state,
        fontSizes: { ...state.fontSizes, [action.key]: action.size },
      };
  }
}

export function createInitialState(): BannerState {
  const template = templates[0];
  const values: Record<string, string> = {};
  for (const field of template.fields) {
    values[field.key] = field.defaultValue;
  }
  return {
    layoutId: template.id,
    platformId: platforms[0].id,
    values,
    fontSizes: {},
  };
}
