import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "./App";

describe("App bootstrap screen", () => {
  it("truthfully reports desktop readiness without implying working features", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { name: "Mistaken" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Desktop runtime ready")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Audio capture and transcription are not configured yet.",
      ),
    ).toBeInTheDocument();

    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });
});
