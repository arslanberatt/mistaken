import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

afterEach(() => {
  cleanup();
});

// jsdom does not implement `Element.prototype.scrollTo`; real browsers and
// WKWebView/WebView2 do (CSSOM View). Tests that assert on `scrollTop`
// still control it directly via `Object.defineProperty`; this stub only
// keeps the auto-follow hook's real, unconditional `scrollTo` call from
// throwing in jsdom.
if (typeof Element.prototype.scrollTo !== "function") {
  Element.prototype.scrollTo = function scrollTo(
    this: HTMLElement,
    options?: ScrollToOptions | number,
  ) {
    if (typeof options === "object" && options !== null && typeof options.top === "number") {
      this.scrollTop = options.top;
    }
  };
}
