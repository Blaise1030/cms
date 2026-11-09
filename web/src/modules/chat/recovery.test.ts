import { describe, expect, it } from "vitest";
import { spawnRecovery } from "./recovery";

describe("spawnRecovery", () => {
  it.each([
    ["spawn cursor ENOENT", "setup"],
    ["claude: command not found", "setup"],
    ["The assistant isn't installed", "setup"],
    ["please sign in to continue", "signIn"],
    ["Not logged in. Run the login flow.", "signIn"],
    ["401 unauthorized", "signIn"],
  ])("maps %j to %s", (message, action) => {
    expect(spawnRecovery(message)).toBe(action);
  });

  it("leaves an unrelated failure alone", () => {
    expect(spawnRecovery("A chat turn is already running")).toBeNull();
    expect(spawnRecovery("temporary failure")).toBeNull();
    expect(spawnRecovery(null)).toBeNull();
  });
});
