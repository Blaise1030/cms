import { describe, expect, it } from "vitest";

import { startGithubStub, STUB_HEAD_SHA, STUB_TOKEN, STUB_USER } from "./githubStub";

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

  it("serves PR state, check-runs, reviews and deployments set via setPrStatus", async () => {
    const stub = await startGithubStub();
    const get = async (p: string) =>
      (await fetch(`${stub.baseUrl}${p}`, { headers: { Authorization: `Bearer ${STUB_TOKEN}` } })).json();
    try {
      await fetch(`${stub.baseUrl}/repos/acme/site/pulls`, {
        method: "POST",
        headers: { Authorization: `Bearer ${STUB_TOKEN}`, "Content-Type": "application/json" },
        body: JSON.stringify({ title: "T", head: "content/t", base: "main", draft: true }),
      });
      expect(await get("/repos/acme/site/deployments?sha=" + STUB_HEAD_SHA)).toEqual([]);

      stub.setPrStatus({
        draft: false,
        merged: true,
        state: "closed",
        checkRuns: [{ status: "completed", conclusion: "success" }],
        reviews: [{ user: { login: "g" }, state: "APPROVED" }],
        deployment: { environment_url: "https://x.pages.dev" },
      });
      const prs = (await get("/repos/acme/site/pulls?head=acme:content/t&state=all")) as {
        draft: boolean;
        state: string;
        merged_at: string | null;
        head: { sha: string };
      }[];
      expect(prs[0]).toMatchObject({ draft: false, state: "closed", head: { sha: STUB_HEAD_SHA } });
      expect(prs[0]!.merged_at).not.toBeNull();
      expect(await get("/repos/acme/site/pulls?head=acme:content/t&state=open")).toEqual([]);
      expect(await get("/repos/acme/site/pulls/1/reviews")).toHaveLength(1);
      expect(await get(`/repos/acme/site/commits/${STUB_HEAD_SHA}/check-runs`)).toMatchObject({
        total_count: 1,
      });
      const deps = (await get("/repos/acme/site/deployments?sha=" + STUB_HEAD_SHA)) as { id: number }[];
      expect(await get(`/repos/acme/site/deployments/${deps[0]!.id}/statuses`)).toEqual([
        { state: "success", environment_url: "https://x.pages.dev" },
      ]);
    } finally {
      await stub.close();
    }
  });
  it("marks a PR ready via the GraphQL markPullRequestReadyForReview mutation", async () => {
    const stub = await startGithubStub();
    const auth = { Authorization: `Bearer ${STUB_TOKEN}`, "Content-Type": "application/json" };
    const gql = (variables: unknown, headers: Record<string, string> = auth) =>
      fetch(`${stub.baseUrl}/graphql`, {
        method: "POST",
        headers,
        body: JSON.stringify({
          query:
            "mutation($id: ID!) { markPullRequestReadyForReview(input: { pullRequestId: $id }) { pullRequest { isDraft } } }",
          variables,
        }),
      });
    try {
      await fetch(`${stub.baseUrl}/repos/acme/site/pulls`, {
        method: "POST",
        headers: auth,
        body: JSON.stringify({ title: "T", head: "content/t", base: "main", draft: true }),
      });
      const listed = (await (
        await fetch(`${stub.baseUrl}/repos/acme/site/pulls?head=acme:content/t`, { headers: auth })
      ).json()) as { node_id: string; draft: boolean }[];
      expect(listed[0]).toMatchObject({ node_id: "PR_kwStub1", draft: true });

      expect((await gql({ id: "PR_kwStub1" }, {})).status).toBe(401);

      const res = await gql({ id: "PR_kwStub1" });
      expect(res.status).toBe(200);
      expect(await res.json()).toEqual({
        data: { markPullRequestReadyForReview: { pullRequest: { isDraft: false } } },
      });
      expect(stub.readyForReviewCalls()).toEqual(["PR_kwStub1"]);
      const after = (await (
        await fetch(`${stub.baseUrl}/repos/acme/site/pulls?head=acme:content/t`, { headers: auth })
      ).json()) as { draft: boolean }[];
      expect(after[0]!.draft).toBe(false);

      const unknown = (await (await gql({ id: "PR_nope" })).json()) as { errors?: unknown[] };
      expect(unknown.errors).toHaveLength(1);

      stub.setGraphqlStatus(403);
      expect((await gql({ id: "PR_kwStub1" })).status).toBe(403);
    } finally {
      await stub.close();
    }
  });
});
