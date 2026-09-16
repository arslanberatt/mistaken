import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { isApplePlatform, useWorkspaceShortcuts } from "./use-workspace-shortcuts";

function dispatch(init: Partial<KeyboardEventInit> & { key: string }): boolean {
  const event = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init });
  return window.dispatchEvent(event);
}

function renderShortcuts(overrides: {
  onToggleCapture?: (() => void) | undefined;
  onCopyAll?: (() => void) | undefined;
  onCancelClearConfirmation?: (() => void) | undefined;
} = {}) {
  const onToggleCapture = "onToggleCapture" in overrides ? overrides.onToggleCapture : vi.fn();
  const onCopyAll = "onCopyAll" in overrides ? overrides.onCopyAll : vi.fn();
  const onCancelClearConfirmation = overrides.onCancelClearConfirmation;
  const utils = renderHook(
    (props: {
      onToggleCapture: (() => void) | undefined;
      onCopyAll: (() => void) | undefined;
      onCancelClearConfirmation: (() => void) | undefined;
    }) => useWorkspaceShortcuts(props),
    { initialProps: { onToggleCapture, onCopyAll, onCancelClearConfirmation } },
  );
  return { ...utils, onToggleCapture, onCopyAll, onCancelClearConfirmation };
}

describe("isApplePlatform", () => {
  it("detects macOS from platform or user agent, and nothing else", () => {
    expect(isApplePlatform({ platform: "MacIntel", userAgent: "" })).toBe(true);
    expect(isApplePlatform({ platform: "", userAgent: "iPhone" })).toBe(true);
    expect(isApplePlatform({ platform: "Win32", userAgent: "Windows NT 10.0" })).toBe(false);
    expect(isApplePlatform({ platform: "Linux x86_64", userAgent: "X11; Linux" })).toBe(false);
  });
});

describe("useWorkspaceShortcuts on macOS", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function renderOnMac(overrides: Parameters<typeof renderShortcuts>[0] = {}) {
    vi.stubGlobal("navigator", { ...navigator, platform: "MacIntel", userAgent: "Macintosh" });
    return renderShortcuts(overrides);
  }

  it("toggles capture on Cmd+Enter and prevents default", () => {
    const { onToggleCapture } = renderOnMac();
    let defaultPrevented = true;
    defaultPrevented = dispatch({ key: "Enter", metaKey: true });
    expect(onToggleCapture).toHaveBeenCalledTimes(1);
    expect(defaultPrevented).toBe(false);
  });

  it("does nothing for Ctrl+Enter (the wrong-platform modifier)", () => {
    const { onToggleCapture } = renderOnMac();
    const notPrevented = dispatch({ key: "Enter", ctrlKey: true });
    expect(onToggleCapture).not.toHaveBeenCalled();
    expect(notPrevented).toBe(true);
  });

  it("copies on Cmd+Shift+C", () => {
    const { onCopyAll } = renderOnMac();
    dispatch({ key: "C", metaKey: true, shiftKey: true });
    expect(onCopyAll).toHaveBeenCalledTimes(1);
  });

  it("ignores Cmd+Enter while the toggle action is unavailable, without throwing or preventing default", () => {
    const { onToggleCapture } = renderOnMac({ onToggleCapture: undefined });
    let notPrevented = false;
    expect(() => {
      notPrevented = dispatch({ key: "Enter", metaKey: true });
    }).not.toThrow();
    expect(onToggleCapture).toBeUndefined();
    expect(notPrevented).toBe(true);
  });

  it("ignores Cmd+Shift+C while Copy All is unavailable, without preventing default", () => {
    const { onCopyAll } = renderOnMac({ onCopyAll: undefined });
    const notPrevented = dispatch({ key: "C", metaKey: true, shiftKey: true });
    expect(onCopyAll).toBeUndefined();
    expect(notPrevented).toBe(true);
  });


  it("ignores repeated key events", () => {
    const { onToggleCapture } = renderOnMac();
    dispatch({ key: "Enter", metaKey: true, repeat: true });
    expect(onToggleCapture).not.toHaveBeenCalled();
  });

  it("ignores events during IME composition", () => {
    const { onCopyAll } = renderOnMac();
    dispatch({ key: "C", metaKey: true, shiftKey: true, isComposing: true });
    expect(onCopyAll).not.toHaveBeenCalled();
  });

  it("cancels the Clear confirmation on Escape only while it is open", () => {
    const cancel = vi.fn();
    const { rerender } = renderOnMac({ onCancelClearConfirmation: undefined });
    const notHandled = dispatch({ key: "Escape" });
    expect(notHandled).toBe(true);

    rerender({ onToggleCapture: vi.fn(), onCopyAll: vi.fn(), onCancelClearConfirmation: cancel });
    const handled = dispatch({ key: "Escape" });
    expect(cancel).toHaveBeenCalledTimes(1);
    expect(handled).toBe(false);
  });

  it("never toggles capture on a bare Space press", () => {
    const { onToggleCapture } = renderOnMac();
    dispatch({ key: " " });
    expect(onToggleCapture).not.toHaveBeenCalled();
  });

  it("passes through every unhandled combination: Cmd+C, Cmd+A, Cmd+V, arrows, and plain letters", () => {
    const { onToggleCapture, onCopyAll } = renderOnMac();
    const matrix: (Partial<KeyboardEventInit> & { key: string })[] = [
      { key: "c", metaKey: true },
      { key: "a", metaKey: true },
      { key: "v", metaKey: true },
      { key: "ArrowDown" },
      { key: "ArrowUp" },
      { key: "Tab" },
      { key: "c" },
      { key: "Enter" },
      { key: "C", shiftKey: true },
    ];
    for (const combo of matrix) {
      const notPrevented = dispatch(combo);
      expect(notPrevented).toBe(true);
    }
    expect(onToggleCapture).not.toHaveBeenCalled();
    expect(onCopyAll).not.toHaveBeenCalled();
  });

  it("registers exactly one window keydown listener and removes it on unmount", () => {
    const addSpy = vi.spyOn(window, "addEventListener");
    const removeSpy = vi.spyOn(window, "removeEventListener");
    const { unmount } = renderOnMac();

    const keydownAdds = addSpy.mock.calls.filter(([type]) => type === "keydown");
    expect(keydownAdds).toHaveLength(1);

    unmount();
    const keydownRemoves = removeSpy.mock.calls.filter(([type]) => type === "keydown");
    expect(keydownRemoves).toHaveLength(1);

    addSpy.mockRestore();
    removeSpy.mockRestore();
  });

  it("isolates a thrown handler: the listener keeps working for the next event", () => {
    const throwing = vi.fn(() => {
      throw new Error("boom");
    });
    const { onCopyAll } = renderOnMac({ onToggleCapture: throwing });

    expect(() => dispatch({ key: "Enter", metaKey: true })).not.toThrow();
    dispatch({ key: "C", metaKey: true, shiftKey: true });
    expect(onCopyAll).toHaveBeenCalledTimes(1);
  });
});

describe("useWorkspaceShortcuts on Windows", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function renderOnWindows(overrides: Parameters<typeof renderShortcuts>[0] = {}) {
    vi.stubGlobal("navigator", { ...navigator, platform: "Win32", userAgent: "Windows NT 10.0" });
    return renderShortcuts(overrides);
  }

  it("toggles capture on Ctrl+Enter", () => {
    const { onToggleCapture } = renderOnWindows();
    dispatch({ key: "Enter", ctrlKey: true });
    expect(onToggleCapture).toHaveBeenCalledTimes(1);
  });

  it("does nothing for Meta+Enter (the wrong-platform modifier)", () => {
    const { onToggleCapture } = renderOnWindows();
    dispatch({ key: "Enter", metaKey: true });
    expect(onToggleCapture).not.toHaveBeenCalled();
  });

  it("copies on Ctrl+Shift+C", () => {
    const { onCopyAll } = renderOnWindows();
    dispatch({ key: "C", ctrlKey: true, shiftKey: true });
    expect(onCopyAll).toHaveBeenCalledTimes(1);
  });
});
