/**
 * Single place where app-wide startup / teardown effects run.
 *
 * Analogue of `apps/desktop/src/main2/lifecycle.tsx#useMain2Lifecycle`. It is
 * intentionally a no-op today: the point is that there is exactly one stable
 * seam in the React tree where future ports plug their startup work in, rather
 * than each feature sprinkling `useEffect`s into the renderer root.
 *
 * Candidate additions (ported from the Tauri app):
 * - Electron-side tab restoration / pin hydration (`useDesktopTabLifecycle`).
 * - STT listener bootstrap once the listener stack lands.
 * - Hoisted updater subscription so `use-update` survives `UpdateBanner`
 *   remounts (today the snapshot is banner-local).
 * - Global shortcut / keyboard manager install.
 *
 * Keep this hook side-effect-only. Return values belong in dedicated hooks or
 * stores; this one is for wiring, not for data.
 */
export function useAppLifecycle(): void {
  // Stable extension point. Add effects here, not in `AppRootContainer` or
  // `main.tsx`.
}
