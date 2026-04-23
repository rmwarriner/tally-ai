import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ArtifactCard } from "./ArtifactCard";

describe("ArtifactCard", () => {
  it("renders title and children in a labelled region", () => {
    render(
      <ArtifactCard title="Account Balances">
        <p>content</p>
      </ArtifactCard>,
    );

    expect(screen.getByRole("region", { name: /account balances/i })).toBeInTheDocument();
    expect(screen.getByText("content")).toBeInTheDocument();
  });

  it("copies inner text to clipboard", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText,
      },
    });

    render(
      <ArtifactCard title="Report">
        <p>Hello ledger</p>
      </ArtifactCard>,
    );

    fireEvent.click(screen.getByRole("button", { name: /copy/i }));
    await Promise.resolve();

    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("Hello ledger"));
  });

  it("renders disabled expand action with tooltip", () => {
    render(
      <ArtifactCard title="Report">
        <p>Body</p>
      </ArtifactCard>,
    );

    const expand = screen.getByRole("button", { name: /expand/i });
    expect(expand).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("tooltip", { name: /full view coming soon/i })).toBeInTheDocument();
  });
});
