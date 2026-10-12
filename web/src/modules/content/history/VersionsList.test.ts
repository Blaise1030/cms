// @vitest-environment happy-dom
import { expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import type { FileVersion } from "@/core/types";
import VersionsList from "./VersionsList.vue";

const versions: FileVersion[] = [
  { sha: "c3", summary: "Fix typo", author: "Ann", ts: 1_760_000_300, path: "a.md", published: true },
  { sha: "b2", summary: "Add intro", author: "Bob", ts: 1_760_000_200, path: "a.md", published: true },
  { sha: "a1", summary: "Draft", author: "Ann", ts: 1_760_000_100, path: "a.md", published: false },
];

const props = { versions, selectedSha: null, loading: false, error: null };

it("renders a row per version with its badge", () => {
  const rows = mount(VersionsList, { props }).findAll('[data-testid="history-version"]');
  expect(rows).toHaveLength(3);
  expect(rows.map((r) => r.find('[data-testid="history-badge"]').text())).toEqual([
    "Current", "Published", "Draft",
  ]);
  expect(rows[1]!.text()).toContain("Bob · Add intro");
});

it("emits select with the clicked version's sha", async () => {
  const wrapper = mount(VersionsList, { props });
  await wrapper.findAll('[data-testid="history-version"]')[1]!.trigger("click");
  expect(wrapper.emitted("select")).toEqual([["b2"]]);
});

it("marks the selected row pressed", () => {
  const rows = mount(VersionsList, { props: { ...props, selectedSha: "b2" } })
    .findAll('[data-testid="history-version"]');
  expect(rows.map((r) => r.attributes("aria-pressed"))).toEqual(["false", "true", "false"]);
});

it("shows an empty state when there is no history", () => {
  const wrapper = mount(VersionsList, { props: { ...props, versions: [] } });
  expect(wrapper.text()).toContain("No history yet");
});

it("shows the error with a Retry that emits retry", async () => {
  const wrapper = mount(VersionsList, { props: { ...props, versions: [], error: "boom" } });
  expect(wrapper.text()).toContain("boom");
  const retry = wrapper.findAll("button").find((b) => b.text() === "Retry")!;
  await retry.trigger("click");
  expect(wrapper.emitted("retry")).toHaveLength(1);
});
