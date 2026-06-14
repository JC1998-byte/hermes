import { describe, expect, it } from "vitest";
import { formatBody } from "./App";

describe("formatBody", () => {
  it("pretty prints JSON responses", () => {
    expect(formatBody("{\"ok\":true}", "application/json")).toBe("{\n  \"ok\": true\n}");
  });

  it("leaves plain text untouched", () => {
    expect(formatBody("hello", "text/plain")).toBe("hello");
  });
});
