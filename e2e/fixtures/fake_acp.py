#!/usr/bin/env python3
"""A minimal ACP agent for e2e tests.

`chat` mode replays the final assistant text of the Cursor stream-json fixture,
edits `about.md`, and reports one edit step. `edit` mode edits `notes.md` and
replies with one short line. `sync` mode takes the latest side of every collided
file, stages everything, and replies; `garbage` mode leaves the collision for the
writer. Every mode ends the turn normally so the daemon checkpoints.
"""

import json
import subprocess
import sys

mode = sys.argv[1] if len(sys.argv) > 1 else "edit"
fixture = sys.argv[2] if len(sys.argv) > 2 else ""


def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def update(session_id, upd):
    send(
        {
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {"sessionId": session_id, "update": upd},
        }
    )


def fixture_reply():
    with open(fixture, encoding="utf-8") as handle:
        entries = [json.loads(line) for line in handle if line.strip()]
    assistant = [e for e in entries if e.get("type") == "assistant"][-1]
    return "".join(part.get("text", "") for part in assistant["message"]["content"])


def append(path, line):
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(line + "\n")


def edit(target, session_id):
    append(target, "Hello from the agent.")
    update(
        session_id,
        {
            "sessionUpdate": "tool_call",
            "toolCallId": "call-edit",
            "title": f"Edit {target}",
            "kind": "edit",
            "status": "completed",
            "locations": [{"path": target}],
            "rawInput": {"path": target},
        },
    )


def reply(session_id, text):
    update(
        session_id,
        {
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": text},
        },
    )


while True:
    line = sys.stdin.readline()
    if not line:
        break
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    method = msg.get("method")
    mid = msg.get("id")
    params = msg.get("params") or {}
    if method == "initialize":
        send(
            {
                "jsonrpc": "2.0",
                "id": mid,
                "result": {
                    "protocolVersion": 1,
                    "agentCapabilities": {},
                    "agentInfo": {"name": "fake-acp-e2e", "version": "0.1.0"},
                    "authMethods": [],
                },
            }
        )
    elif method == "session/new":
        send(
            {
                "jsonrpc": "2.0",
                "id": mid,
                "result": {"sessionId": "fake-session-1"},
            }
        )
    elif method == "session/set_config_option":
        send({"jsonrpc": "2.0", "id": mid, "result": {}})
    elif method == "session/prompt":
        session_id = params.get("sessionId", "")
        if mode == "chat":
            edit("about.md", session_id)
            reply(session_id, fixture_reply())
        elif mode == "sync":
            # Resolve the merge the daemon left in the worktree: keep the latest
            # side of each collided page and stage the result.
            subprocess.run(
                ["sh", "-c", "git checkout --theirs -- $(git diff --name-only --diff-filter=U) && git add -A"],
                check=False,
            )
            reply(session_id, "I kept both changes and the site still works.")
        elif mode == "garbage":
            # Give up mid-way: every collided page still visibly needs a decision.
            unmerged = subprocess.run(
                ["git", "diff", "--name-only", "--diff-filter=U"],
                capture_output=True,
                text=True,
                check=False,
            ).stdout
            for path in unmerged.splitlines():
                with open(path, "a", encoding="utf-8") as handle:
                    handle.write("<<<<<<< yours\n=======\n>>>>>>> latest\n")
            reply(session_id, "I couldn't settle these pages.")
        else:
            edit("notes.md", session_id)
            reply(session_id, "Updated the notes.")
        send({"jsonrpc": "2.0", "id": mid, "result": {"stopReason": "end_turn"}})
    elif method == "session/cancel":
        # Notification: no response. A real agent would abort the prompt.
        pass
    elif mid is not None:
        send({"jsonrpc": "2.0", "id": mid, "result": {}})
