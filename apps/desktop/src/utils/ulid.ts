import { ulid } from "ulid";

export function generateUlid(): string {
  return ulid();
}
