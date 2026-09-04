import { fireEvent } from "@testing-library/react";

export function completeDestructiveButtonHold(button: HTMLElement) {
  fireEvent.pointerDown(button, { button: 0, isPrimary: true });

  finishDestructiveButtonHold(button);
}

export function finishDestructiveButtonHold(button: HTMLElement) {
  const event = new Event("animationend", { bubbles: true });
  Object.defineProperty(event, "animationName", {
    value: "destructive-button-hold",
  });
  fireEvent(button, event);
}
