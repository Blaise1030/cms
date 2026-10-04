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
});
