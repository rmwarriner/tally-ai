import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import styles from "./SystemMessage.module.css";
import { SystemMessage } from "./SystemMessage";

describe("SystemMessage", () => {
  it("renders centered system text", () => {
    render(<SystemMessage text="Last transaction undone." />);
    expect(screen.getByText("Last transaction undone.")).toBeInTheDocument();
  });

  it("applies error styling for error tone", () => {
    render(<SystemMessage text="Unknown command." tone="error" />);
    expect(screen.getByText("Unknown command.")).toHaveClass(styles.error);
  });
});
