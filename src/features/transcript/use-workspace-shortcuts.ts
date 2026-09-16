/**
 * The single window-level keyboard-shortcut listener for the workspace:
 * platform-correct `Cmd/Ctrl + Enter` toggles capture, `Cmd/Ctrl + Shift +
 * C` copies, and `Escape` cancels an open Clear confirmation. Every
 * handler is the exact function the corresponding button already calls —
 * this hook adds no alternate code path or disabled-state duplication.
 *
 * `preventDefault()` is called **only** when a handler for the matched
 * combination is actually defined, i.e. the action is currently
 * available. When `onToggleCapture`/`onCopyAll` is `undefined` (the
 * action is disabled or already pending), the event is left completely
 * untouched — the same as any other unhandled combination — so it never
 * silently swallows a native shortcut (notably the development-webview
 * devtools-inspector collision on `Cmd/Ctrl+Shift+C`) for an action that
 * produced no visible effect. A thrown handler is caught so one failure
 * can never unregister the listener.
 */
import { useEffect, useRef } from "react";

export interface WorkspaceShortcutHandlers {
  readonly onToggleCapture: (() => void) | undefined;
  readonly onCopyAll: (() => void) | undefined;
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
          const toggle = handlersRef.current.onToggleCapture;
          if (toggle) {
            event.preventDefault();
            toggle();
          }
          return;
        }

        if (event.shiftKey && event.key.toLowerCase() === "c") {
          const copy = handlersRef.current.onCopyAll;
          if (copy) {
            event.preventDefault();
            copy();
          }
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
