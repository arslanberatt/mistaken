import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { NEAR_BOTTOM_THRESHOLD_PX, useAutoFollow } from "./use-auto-follow";

function makeScrollableDiv(): HTMLDivElement {
  const div = document.createElement("div");
  document.body.appendChild(div);
  Object.defineProperty(div, "scrollHeight", { value: 1000, configurable: true });
  Object.defineProperty(div, "clientHeight", { value: 400, configurable: true });
  Object.defineProperty(div, "scrollTop", { value: 0, configurable: true, writable: true });
  return div;
}

function setScrollTop(el: HTMLElement, value: number) {
  Object.defineProperty(el, "scrollTop", { value, configurable: true, writable: true });
}

function setScrollHeight(el: HTMLElement, value: number) {
  Object.defineProperty(el, "scrollHeight", { value, configurable: true, writable: true });
}

/**
 * Dispatches a scroll event and flushes the hook's one-frame `rAF`
 * coalescer. `requestAnimationFrame` is stubbed onto a microtask (see
 * `beforeEach`) so the hook's own scheduling-then-flagging order — which
 * only holds when the frame callback runs strictly after the scheduling
 * call returns, exactly as real browsers behave — is exercised honestly,
 * without a real wall-clock wait.
 */
async function scrollAndFlush(el: HTMLElement) {
  await act(async () => {
    el.dispatchEvent(new Event("scroll"));
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(() => {
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
    queueMicrotask(() => cb(0));
    return 1;
  });
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

describe("useAutoFollow", () => {
  it("follows by default and scrolls to bottom when followKey changes", () => {
    const div = makeScrollableDiv();
    const containerRef = { current: div };
    const scrollToSpy = vi.spyOn(div, "scrollTo");

    const { result, rerender } = renderHook(
      ({ key }) => useAutoFollow(containerRef, key),
      { initialProps: { key: 1 } },
    );

    expect(result.current.isFollowing).toBe(true);
    rerender({ key: 2 });
    expect(scrollToSpy).toHaveBeenCalledWith(
      expect.objectContaining({ top: div.scrollHeight }),
    );
  });

  it("detaches the moment the user scrolls past the 64px threshold", async () => {
    const div = makeScrollableDiv();
    const containerRef = { current: div };
    const { result } = renderHook(() => useAutoFollow(containerRef, 0));

    setScrollTop(div, 1000 - 400 - (NEAR_BOTTOM_THRESHOLD_PX + 1));
    await scrollAndFlush(div);

    expect(result.current.isFollowing).toBe(false);
  });

  it("stays following exactly at the 64px threshold", async () => {
    const div = makeScrollableDiv();
    const containerRef = { current: div };
    const { result } = renderHook(() => useAutoFollow(containerRef, 0));

    setScrollTop(div, 1000 - 400 - NEAR_BOTTOM_THRESHOLD_PX);
    await scrollAndFlush(div);

    expect(result.current.isFollowing).toBe(true);
  });

  it("never touches scrollTop while detached, including across interim growth", async () => {
    const div = makeScrollableDiv();
    const containerRef = { current: div };
    const scrollToSpy = vi.spyOn(div, "scrollTo");
    const { rerender } = renderHook(
      ({ key }) => useAutoFollow(containerRef, key),
      { initialProps: { key: 0 } },
    );

    setScrollTop(div, 100);
    await scrollAndFlush(div);
    scrollToSpy.mockClear();

    // A growing interim row (or a new final) changes followKey/scrollHeight
    // while detached; neither may move scrollTop.
    setScrollHeight(div, 1400);
    rerender({ key: 1 });
    rerender({ key: 2 });

    expect(scrollToSpy).not.toHaveBeenCalled();
    expect(div.scrollTop).toBe(100);
  });

  it("restores follow when the user scrolls back within the threshold", async () => {
    const div = makeScrollableDiv();
    const containerRef = { current: div };
    const { result } = renderHook(() => useAutoFollow(containerRef, 0));

    setScrollTop(div, 500);
    await scrollAndFlush(div);
    expect(result.current.isFollowing).toBe(false);

    setScrollTop(div, 1000 - 400 - 10);
    await scrollAndFlush(div);
    expect(result.current.isFollowing).toBe(true);
  });

  it("jumpToLatest restores follow and scrolls to bottom", async () => {
    const div = makeScrollableDiv();
    const containerRef = { current: div };
    const scrollToSpy = vi.spyOn(div, "scrollTo");
    const { result } = renderHook(() => useAutoFollow(containerRef, 0));

    setScrollTop(div, 500);
    await scrollAndFlush(div);
    expect(result.current.isFollowing).toBe(false);

    act(() => {
      result.current.jumpToLatest();
    });

    expect(result.current.isFollowing).toBe(true);
    expect(scrollToSpy).toHaveBeenCalledWith(
      expect.objectContaining({ top: div.scrollHeight }),
    );
  });

  it("uses instant scrolling when the user prefers reduced motion", () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query.includes("prefers-reduced-motion"),
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    const div = makeScrollableDiv();
    const containerRef = { current: div };
    const scrollToSpy = vi.spyOn(div, "scrollTo");
    const { rerender } = renderHook(
      ({ key }) => useAutoFollow(containerRef, key),
      { initialProps: { key: 0 } },
    );

    rerender({ key: 1 });
    expect(scrollToSpy).toHaveBeenCalledWith(
      expect.objectContaining({ behavior: "auto" }),
    );
  });

  it("registers exactly one scroll listener and cleans it up on unmount", () => {
    const div = makeScrollableDiv();
    const containerRef = { current: div };
    const addSpy = vi.spyOn(div, "addEventListener");
    const removeSpy = vi.spyOn(div, "removeEventListener");

    const { unmount } = renderHook(() => useAutoFollow(containerRef, 0));
    expect(addSpy.mock.calls.filter(([type]) => type === "scroll")).toHaveLength(1);

    unmount();
    expect(removeSpy.mock.calls.filter(([type]) => type === "scroll")).toHaveLength(1);
  });
});
