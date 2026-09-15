/**
 * The single window-level keyboard-shortcut listener for the workspace:
 * platform-correct `Cmd/Ctrl + Enter` toggles capture, `Cmd/Ctrl + Shift +
 * C` copies, and `Escape` cancels an open Clear confirmation. Every
 * handler is the exact function the corresponding button already calls —
 * this hook adds no alternate code path or disabled-state duplication,
 * since a `undefined` action is simply not invoked and each callback's
 * own guard (pending/disabled) governs the rest.
 *
 * `preventDefault()` is called only for a recognized combination, so
 * native selection, platform copy/select-all, and text navigation inside
 * the transcript are never touched for anything else. A thrown handler is
 * caught so one failure can never unregister the listener.
 */
import { useEffect, useRef } from "react";

export interface WorkspaceShortcutHandlers {
  readonly onToggleCapture: (() => void) | undefined;
  readonly onCopyAll: () => void;
  readonly onCancelClearConfirmation: (() => void) | undefined;
}

export function isApplePlatform(
  nav: Pick<Navigator, "platform" | "userAgent"> = navigator,
): boolean {
  return /Mac|iPhone|iPad|iPod/.test(nav.platform || nav.userAgent || "");
}

export function useWorkspaceShortcuts(handlers: WorkspaceShortcutHandlers): void {
  const handlersRef = useRef(handlers);
  useEffect(() => {
    handlersRef.current = handlers;
  });

  useEffect(() => {
    const isMac = isApplePlatform();

    function handleKeyDown(event: KeyboardEvent) {
      try {
        if (event.repeat || event.isComposing) {
          return;
        }

        if (event.key === "Escape") {
          const cancel = handlersRef.current.onCancelClearConfirmation;
          if (cancel) {
            event.preventDefault();
            cancel();
          }
          return;
        }

        const platformModifierPressed = isMac ? event.metaKey : event.ctrlKey;
        if (!platformModifierPressed) {
          return;
        }

        if (event.key === "Enter") {
          event.preventDefault();
          handlersRef.current.onToggleCapture?.();
          return;
        }

        if (event.shiftKey && event.key.toLowerCase() === "c") {
          event.preventDefault();
          handlersRef.current.onCopyAll();
        }
      } catch {
        // A handler failure must never unregister this listener or hang
        // the app; any resulting failure surfaces through the existing
        // feedback path (e.g. copy-failure state), not here.
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);
}
