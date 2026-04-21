import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { SidebarToggle } from "./SidebarToggle";

const originalUserAgent = navigator.userAgent;

afterEach(() => {
  Object.defineProperty(window.navigator, "userAgent", {
    configurable: true,
    value: originalUserAgent,
  });
});

describe("SidebarToggle", () => {
  it("shows collapse label and Cmd+B tooltip on macOS", () => {
    Object.defineProperty(window.navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Macintosh; Intel Mac OS X)",
    });

    render(<SidebarToggle open onToggle={vi.fn()} />);

    expect(screen.getByRole("button", { name: /collapse sidebar/i })).toBeInTheDocument();
    expect(screen.getByRole("tooltip", { name: /cmd\+b/i })).toBeInTheDocument();
  });

  it("shows expand label and Ctrl+B tooltip on non-macOS", () => {
    Object.defineProperty(window.navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });

    render(<SidebarToggle open={false} onToggle={vi.fn()} />);

    expect(screen.getByRole("button", { name: /expand sidebar/i })).toBeInTheDocument();
    expect(screen.getByRole("tooltip", { name: /ctrl\+b/i })).toBeInTheDocument();
  });
});
