import { describe, expect, it } from "vitest";

import { startGithubStub, STUB_TOKEN, STUB_USER } from "./githubStub";

describe("githubStub", () => {
  it("serves /user and device-flow with a signed-in identity", async () => {
    const stub = await startGithubStub();
    try {
      const device = await fetch(`${stub.baseUrl}/login/device/code`, {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: "client_id=cid",
      });
      expect(device.ok).toBe(true);

      const poll = async () =>
        fetch(`${stub.baseUrl}/login/oauth/access_token`, {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body: "device_code=dev123",
        });

      expect(await (await poll()).json()).toMatchObject({ error: "authorization_pending" });
      const tokenRes = await poll();
      expect(await tokenRes.json()).toMatchObject({
        access_token: STUB_TOKEN,
        token_type: "bearer",
      });

      const user = await fetch(`${stub.baseUrl}/user`, {
        headers: { Authorization: `Bearer ${STUB_TOKEN}` },
      });
      expect(user.ok).toBe(true);
      expect(await user.json()).toMatchObject(STUB_USER);
    } finally {
      await stub.close();
    }
  });

  it("records draft PR creates and finds them by head", async () => {
    const stub = await startGithubStub();
    try {
      const create = await fetch(`${stub.baseUrl}/repos/acme/site/pulls`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${STUB_TOKEN}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          title: "Launch week",
          head: "content/launch-week",
          base: "main",
          draft: true,
        }),
      });
      expect(create.status).toBe(201);
      expect(stub.pullsPosts()).toHaveLength(1);
      expect(stub.pullsPosts()[0]!.body.draft).toBe(true);

      const list = await fetch(
        `${stub.baseUrl}/repos/acme/site/pulls?head=acme:content/launch-week`,
        { headers: { Authorization: `Bearer ${STUB_TOKEN}` } },
      );
      expect(list.ok).toBe(true);
      const prs = (await list.json()) as { number: number }[];
      expect(prs).toHaveLength(1);
      expect(prs[0]!.number).toBe(1);
    } finally {
      await stub.close();
    }
  });

  it("serves main branch protection as 200 or 404", async () => {
    const stub = await startGithubStub();
    try {
      const get = () =>
        fetch(`${stub.baseUrl}/repos/acme/site/branches/main/protection`, {
          headers: { Authorization: `Bearer ${STUB_TOKEN}` },
        });
      expect((await get()).status).toBe(200);
      stub.setMainProtected(false);
      const res = await get();
      expect(res.status).toBe(404);
      expect(await res.json()).toMatchObject({ message: "Branch not protected" });
      expect(stub.protectionGets()).toBe(2);
    } finally {
      await stub.close();
    }
  });

  it("serves main branch rules, with a pull_request rule on demand", async () => {
    const stub = await startGithubStub();
    try {
      const get = async () =>
        (await (
          await fetch(`${stub.baseUrl}/repos/acme/site/rules/branches/main`, {
            headers: { Authorization: `Bearer ${STUB_TOKEN}` },
          })
        ).json()) as { type: string }[];
      expect(await get()).toEqual([]);
      stub.setMainRulesetRequiresPr(true);
      expect((await get()).map((r) => r.type)).toEqual(["pull_request"]);
    } finally {
      await stub.close();
    }
  });
});
