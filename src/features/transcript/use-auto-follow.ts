/**
 * Near-bottom auto-follow for the transcript region. Follow is on at
 * mount and whenever the container is within `NEAR_BOTTOM_THRESHOLD_PX`
 * of the bottom; any user scroll past that threshold detaches it
 * immediately. While detached, this hook never touches `scrollTop` on its
 * own — new segments, interim growth, and status changes cannot move it,
 * because the programmatic-scroll effect below is a no-op whenever
 * follow is off. Exactly one `scroll` listener and one outstanding
 * `requestAnimationFrame` coalescer exist, both cleaned up on unmount.
 */
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { usePrefersReducedMotion } from "./use-prefers-reduced-motion";

export const NEAR_BOTTOM_THRESHOLD_PX = 64;

export interface UseAutoFollowResult {
  readonly isFollowing: boolean;
  readonly jumpToLatest: () => void;
}

export function useAutoFollow(
  containerRef: RefObject<HTMLElement | null>,
  followKey: unknown,
): UseAutoFollowResult {
  const [isFollowing, setIsFollowing] = useState(true);
  const isFollowingRef = useRef(true);
  const pendingFrameRef = useRef<number | null>(null);
  const prefersReducedMotion = usePrefersReducedMotion();
  const prefersReducedMotionRef = useRef(prefersReducedMotion);
  prefersReducedMotionRef.current = prefersReducedMotion;

  const scrollToBottom = useCallback(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }
    container.scrollTo({
      top: container.scrollHeight,
      behavior: prefersReducedMotionRef.current ? "auto" : "smooth",
    });
  }, [containerRef]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    function handleScroll() {
      if (pendingFrameRef.current !== null) {
        return;
      }
      pendingFrameRef.current = requestAnimationFrame(() => {
        pendingFrameRef.current = null;
        const element = containerRef.current;
        if (!element) {
          return;
        }
        const distanceFromBottom =
          element.scrollHeight - element.scrollTop - element.clientHeight;
        const nearBottom = distanceFromBottom <= NEAR_BOTTOM_THRESHOLD_PX;
        if (nearBottom !== isFollowingRef.current) {
          isFollowingRef.current = nearBottom;
          setIsFollowing(nearBottom);
        }
      });
    }

    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => {
      container.removeEventListener("scroll", handleScroll);
      if (pendingFrameRef.current !== null) {
        cancelAnimationFrame(pendingFrameRef.current);
        pendingFrameRef.current = null;
      }
    };
  }, [containerRef]);

  // Only ever scrolls while following; a detached container is left
  // completely untouched regardless of what `followKey` changed to.
  useLayoutEffect(() => {
    if (isFollowingRef.current) {
      scrollToBottom();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [followKey]);

  const jumpToLatest = useCallback(() => {
    isFollowingRef.current = true;
    setIsFollowing(true);
    scrollToBottom();
  }, [scrollToBottom]);

  return { isFollowing, jumpToLatest };
}
