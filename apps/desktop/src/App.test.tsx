import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import App from "./App";

describe("App", () => {
  it("renders app shell with sidebar and chat regions", () => {
    render(<App />);

    expect(screen.getByRole("complementary", { name: /financial health/i })).toBeInTheDocument();
    expect(screen.getByRole("log", { name: /chat thread/i })).toBeInTheDocument();
  });

  it("Cmd/Ctrl+B cycles sidebar width open -> icon -> hidden -> open", () => {
    render(<App />);

    const sidebar = screen.getByRole("complementary", { name: /financial health/i });
    expect(sidebar).toHaveStyle({ width: "280px" });

    fireEvent.keyDown(window, { key: "b", metaKey: true });
    expect(sidebar).toHaveStyle({ width: "48px" });

    fireEvent.keyDown(window, { key: "b", ctrlKey: true });
    expect(sidebar).toHaveStyle({ width: "0px" });

    fireEvent.keyDown(window, { key: "b", ctrlKey: true });
    expect(sidebar).toHaveStyle({ width: "280px" });
  });
});
