import { describe, it, expect } from "vitest";
import { generateUlid } from "./ulid";

describe("generateUlid", () => {
  it("generates a 26-char string", () => {
    const id = generateUlid();
    expect(id).toHaveLength(26);
  });

  it("generates unique IDs", () => {
    const id1 = generateUlid();
    const id2 = generateUlid();
    expect(id1).not.toBe(id2);
  });

  it("generates lexicographically sortable IDs with timestamp ordering", () => {
    const ids: string[] = [];
    for (let i = 0; i < 10; i++) {
      ids.push(generateUlid());
      // Small delay to ensure different timestamps
      // eslint-disable-next-line no-empty
      let t = Date.now();
      while (Date.now() - t < 1) {}
    }
    const sorted = [...ids].sort();
    expect(sorted).toEqual(ids);
  });

  it("generates uppercase strings", () => {
    const id = generateUlid();
    expect(id).toBe(id.toUpperCase());
  });
});
