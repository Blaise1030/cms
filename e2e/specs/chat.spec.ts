import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { CURSOR_TURN_FIXTURE, startChange } from "../fixtures/daemon";
import { expect, test } from "../fixtures/orso";

/** The fixture turn's final assistant text, which is both the reply and the checkpoint summary. */
function fixtureReply(): string {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
  const lines = fs.readFileSync(path.join(root, CURSOR_TURN_FIXTURE), "utf8").trim().split("\n");
  const assistant = lines.map((l) => JSON.parse(l)).filter((e) => e.type === "assistant").at(-1);
  return assistant.message.content.map((c: { text: string }) => c.text).join("");
}

/** The daemon's checkpoint subject: the first line, cut to 72 characters with an ellipsis. */
function checkpointSummary(text: string): string {
  const chars = Array.from(text.trim().split("\n")[0]!.trim());
  return chars.length <= 72 ? chars.join("") : `${chars.slice(0, 71).join("")}…`;
}

test("chat sends a message, shows the reply and a checkpoint", async ({ page, orso }) => {
  const { origin, token, projectId } = orso;
  const reply = fixtureReply();

  await page.goto(`${origin}/?t=${token}`);
  await startChange(orso, "Chat e2e");
  await page.goto(`${origin}/p/${projectId}`);
  await expect(page.getByTestId("preview-panel")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("chat-panel")).toBeVisible();
  await expect(page.getByTestId("agent-panel").getByRole("tablist")).toHaveCount(0);

  await page.getByTestId("chat-provider").selectOption("cursor");
  await page.getByTestId("chat-input").fill("hello");
  await page.getByTestId("chat-send").click();

  const panel = page.getByTestId("chat-panel");
  // The reply renders as markdown, so its inline code loses the backticks.
  await expect(panel).toContainText(reply.replaceAll("`", ""), { timeout: 60_000 });
  await expect(panel.getByText(/^Edited about\.md/)).toBeVisible();
  await expect(panel.getByText("Saved 1 file")).toBeVisible({ timeout: 60_000 });

  await page.goto(`${origin}/p/${projectId}/history`);
  await expect(page.getByTestId("checkpoints-panel")).toBeVisible();
  await expect(page.getByTestId("checkpoint-item").first()).toContainText(checkpointSummary(reply), {
    timeout: 60_000,
  });
});
