/**
 * Live `prefers-reduced-motion` read backing the auto-follow scroll
 * behavior. Registers exactly one `change` listener on the media query
 * list for the hook's lifetime, so toggling the OS setting mid-session
 * takes effect without a relaunch. If `matchMedia` itself is unavailable,
 * this conservatively reports `true`, because instant scrolling is the
 * accessible default when the preference cannot be read.
 */
import { useEffect, useState } from "react";

const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

function readPreference(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return true;
  }
  return window.matchMedia(REDUCED_MOTION_QUERY).matches;
}

export function usePrefersReducedMotion(): boolean {
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(readPreference);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }

    const mediaQueryList = window.matchMedia(REDUCED_MOTION_QUERY);

    function handleChange(event: MediaQueryListEvent) {
      setPrefersReducedMotion(event.matches);
    }

    mediaQueryList.addEventListener("change", handleChange);
    return () => mediaQueryList.removeEventListener("change", handleChange);
  }, []);

  return prefersReducedMotion;
}
