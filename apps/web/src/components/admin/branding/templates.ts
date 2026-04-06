import type { CSSProperties } from "react";

import { colors, fonts } from "./tokens";

export type FieldDef = {
  key: string;
  label: string;
  type:
    | "text"
    | "textarea"
    | "color"
    | "select"
    | "image"
    | "range"
    | "toggle"
    | "toggle-group";
  options?: string[];
  min?: number;
  max?: number;
  step?: number;
  defaultValue: string;
};

export type RegionResize = "top-right" | "bottom-right" | false;

export type Region = {
  key: string;
  type: "text" | "shape" | "image" | "logo" | "avatar" | "author-block";
  editable: boolean;
  style: CSSProperties;
  visibilityKey?: string;
  resize?: RegionResize;
};

export type LayoutTemplate = {
  id: string;
  label: string;
  description: string;
  fields: FieldDef[];
  regions: Region[];
  canvasStyle: CSSProperties;
  hasBgImage?: boolean;
  overlayOpacity?: number;
};

export const templates: LayoutTemplate[] = [
  {
    id: "testimonial",
    label: "Testimonial",
    description: "Quote card with avatar",
    fields: [
      {
        key: "quote",
        label: "Quote",
        type: "textarea",
        defaultValue:
          "there is some text as example about something that user says",
      },
      {
        key: "author",
        label: "Name",
        type: "text",
        defaultValue: "User name",
      },
      {
        key: "role",
        label: "Position / Company",
        type: "text",
        defaultValue: "position or company (optional)",
      },
      {
        key: "avatar",
        label: "Avatar",
        type: "image",
        defaultValue: "",
      },
      {
        key: "pageNum",
        label: "Page Number",
        type: "text",
        defaultValue: "1 of 42",
      },
      {
        key: "showPageNum",
        label: "Show Page Number",
        type: "toggle",
        defaultValue: "true",
      },
      {
        key: "logoVariant",
        label: "Logo Style",
        type: "toggle-group",
        options: ["compact", "full"],
        defaultValue: "compact",
      },
      {
        key: "bgColor",
        label: "Background",
        type: "color",
        defaultValue: colors.page,
      },
      {
        key: "textColor",
        label: "Text Color",
        type: "color",
        defaultValue: colors.fg,
      },
    ],
    regions: [
      {
        key: "contentPanel",
        type: "shape",
        editable: false,
        style: {
          position: "absolute",
          top: "24px",
          left: "24px",
          right: "24px",
          bottom: "24px",
          borderRadius: "0",
          backgroundColor: "#ffffff",
          border: `1px solid ${colors.grey300}`,
        },
      },
      {
        key: "logo",
        type: "logo",
        editable: false,
        style: {
          position: "absolute",
          top: "7%",
          left: "80px",
          height: "48px",
        },
      },
      {
        key: "logoSeparator",
        type: "shape",
        editable: false,
        style: {
          position: "absolute",
          top: "14%",
          left: "24px",
          right: "24px",
          height: "1px",
          backgroundColor: colors.grey300,
        },
      },
      {
        key: "pageNum",
        type: "text",
        editable: true,
        resize: false,
        style: {
          position: "absolute",
          top: "7%",
          right: "80px",
          fontSize: "24px",
          fontWeight: "400",
          fontFamily: fonts.sans,
          opacity: 0.4,
        },
        visibilityKey: "showPageNum",
      },
      {
        key: "quotemark",
        type: "shape",
        editable: false,
        style: {
          position: "absolute",
          top: "17%",
          left: "7%",
          fontSize: "64px",
          fontWeight: "400",
          fontFamily: fonts.serif,
          lineHeight: "1",
          opacity: 0.3,
        },
      },
      {
        key: "quote",
        type: "text",
        editable: true,
        resize: "bottom-right",
        style: {
          position: "absolute",
          top: "17%",
          left: "12%",
          right: "80px",
          fontSize: "52px",
          fontWeight: "500",
          fontFamily: fonts.sans,
          lineHeight: "1.25",
        },
      },
      {
        key: "authorBlock",
        type: "author-block",
        editable: false,
        style: {
          position: "absolute",
          bottom: "7%",
          left: "7%",
          right: "7%",
          display: "flex",
          alignItems: "center",
          gap: "12px",
        },
      },
    ],
    canvasStyle: {},
  },
  {
    id: "post",
    label: "Social Post",
    description: "Background image with text overlay",
    hasBgImage: true,
    overlayOpacity: 0.45,
    fields: [
      {
        key: "bgImage",
        label: "Background Image URL",
        type: "image",
        defaultValue: "",
      },
      {
        key: "body",
        label: "Text",
        type: "textarea",
        defaultValue:
          "hey char what did we discuss last week about our upcoming sprint?",
      },
      {
        key: "overlayOpacity",
        label: "Overlay Opacity",
        type: "range",
        min: 0,
        max: 1,
        step: 0.05,
        defaultValue: "0.45",
      },
      {
        key: "textColor",
        label: "Text Color",
        type: "color",
        defaultValue: "#ffffff",
      },
    ],
    regions: [
      {
        key: "logo",
        type: "logo",
        editable: false,
        style: {
          position: "absolute",
          top: "6%",
          left: "6%",
          height: "48px",
        },
      },
      {
        key: "logoSeparator",
        type: "shape",
        editable: false,
        style: {
          position: "absolute",
          top: "13%",
          left: "0",
          right: "0",
          height: "1px",
          backgroundColor: "rgba(255,255,255,0.2)",
        },
      },
      {
        key: "body",
        type: "text",
        editable: true,
        resize: "top-right",
        style: {
          position: "absolute",
          bottom: "8%",
          left: "6%",
          right: "10%",
          fontSize: "42px",
          fontWeight: "400",
          fontFamily: fonts.mono,
          lineHeight: "1.3",
        },
      },
    ],
    canvasStyle: {},
  },
];
