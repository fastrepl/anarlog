export class AppLockController {
  private state = { locked: true, covered: false, authenticating: false };
  private backgroundedDuringAuthentication = false;
  private listeners = new Set<() => void>();

  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private update(changes: Partial<typeof this.state>) {
    this.state = { ...this.state, ...changes };
    this.listeners.forEach((listener) => listener());
  }
  appStateChanged(state: string) {
    if (state === "background" && this.state.authenticating)
      this.backgroundedDuringAuthentication = true;
    this.update({
      covered: state !== "active",
      ...(state === "background" ||
      (state === "inactive" && !this.state.authenticating)
        ? { locked: true }
        : {}),
    });
  }
  beginAuthentication() {
    if (this.state.authenticating) return false;
    this.backgroundedDuringAuthentication = false;
    this.update({ authenticating: true });
    return true;
  }
  finishAuthentication(success: boolean) {
    this.update({
      authenticating: false,
      ...(success && !this.backgroundedDuringAuthentication
        ? { locked: false }
        : {}),
    });
  }
}
