import "@testing-library/jest-dom/vitest";
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ErrorBoundary } from "./ErrorBoundary";

function Boom(): null {
  throw new Error("kaboom");
}

describe("ErrorBoundary", () => {
  it("renders children when no error", () => {
    render(<ErrorBoundary><div>hello</div></ErrorBoundary>);
    expect(screen.getByText("hello")).toBeInTheDocument();
  });

  it("renders a system message with Get help action when a child throws", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(<ErrorBoundary><Boom /></ErrorBoundary>);
    expect(screen.getByText(/something went wrong/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /get help/i })).toBeInTheDocument();
    spy.mockRestore();
  });
});
