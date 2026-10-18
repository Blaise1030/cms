# Chat translator fixtures

Headless JSON transcripts from each coding CLI, one event per line, used to unit-test the
translators that turn CLI output into chat events. Recorded 2026-10-17.

## Recorded vs hand-written

| File | Source |
|------|--------|
| `cursor/turn.jsonl`, `cursor/resume.jsonl`, `cursor/tool-error.jsonl` | Recorded |
| `codex/turn.jsonl`, `codex/resume.jsonl`, `codex/tool-error.jsonl` | Recorded |
| `claude/login-expired.jsonl` | Recorded (a real run with an expired OAuth login) |
| `claude/turn.jsonl`, `claude/resume.jsonl`, `claude/tool-error.jsonl` | **Hand-written** |

Claude was not logged in, so its turn, resume and tool-error fixtures are hand-written from Claude
Code's documented `--output-format stream-json --verbose` shape, with field names taken from the
recorded `login-expired.jsonl` where they overlap. Replace them with recordings once Claude is
logged in.

The `terminal_reason` of a successful Claude turn is **unverified**: the hand-written fixtures
assume `"completed"`. The only observed value is `"api_error"` (in `login-expired.jsonl`).

## CLI versions

- `cursor-agent` 2026.10.16-e3cb207 (logged in, `apiKeySource: "login"`, model `Auto`)
- `codex` codex-cli 0.162.1 (ChatGPT login, no `OPENAI_API_KEY`)
- `claude` 2.1.301 (OAuth session expired)

## How they were recorded

Each CLI ran in a fresh scratch git repo (`/tmp/wb-fixtures/{cli}`, scrubbed to `/work/project`)
holding one commit with `about.md` = `# About\n`. The two turn runs inherited the agent shell's
stdin; the other runs had stdin from `/dev/null` (Codex prints "Reading additional input from
stdin..." to stderr otherwise, so the daemon should close stdin).

Prompts:

- turn: ``Append the line "Hello from the agent." to about.md, then run `ls` and reply with one short sentence.``
- resume: `What line did you add in the previous turn? Answer in one sentence without using tools.`
- tool-error: ``Run `cat missing.txt` and tell me what happened.``

Argv (`<p>` is the prompt; the resume id is the session or thread id from that CLI's turn run):

| CLI | turn / tool-error | resume |
|-----|-------------------|--------|
| Cursor | `cursor-agent -p --force --output-format stream-json <p>` | `cursor-agent -p --force --output-format stream-json --resume <session_id> <p>` |
| Codex | `codex exec --json --dangerously-bypass-approvals-and-sandbox <p>` | `codex exec --json --dangerously-bypass-approvals-and-sandbox -c developer_instructions="x" resume <thread_id> <p>` |

All six runs exited 0. The Codex resume order above (`-c developer_instructions=…` **before**
`resume`) works: the CLI accepted it and the answer referred to the previous turn.

`claude/login-expired.jsonl` came from
`claude -p <p> --output-format stream-json --verbose --dangerously-skip-permissions`.

## Things the recordings show

- Cursor emitted no `thinking/*` events in these runs. Tool kinds seen: `readToolCall`,
  `editToolCall` (with `result.success.diffString`), `shellToolCall` (failure under
  `result.failure`). `call_id` is on the event; `tool_call.toolCallId` is inside.
- Cursor's `ls` lists a `CLAUDE.md` that one of the recording user's hooks created in the scratch
  repo; the Cursor `tool_call` events also carry `hookAdditionalContexts` from those hooks.
- Codex appended the line with a shell `printf … >> about.md`, so there is no file-change item,
  only `command_execution` and `agent_message` items. It also read a skill file from the home
  directory and ran the user's own `tabsh` status command (from their global agent
  instructions); both are kept as realistic extra `command_execution` items.
- Claude's expired-login `result` has `subtype: "success"` **and** `is_error: true`, with
  `terminal_reason: "api_error"`; the assistant message has `error: "authentication_failed"` and
  `is_api_error_message: true`.

## Scrub rules

Applied to every string value in the recorded files:

1. The home directory becomes `/home/user`, and the username becomes `user`.
2. Scratch paths (`/tmp/wb-fixtures/{cli}`, `/private/tmp/…`, and the spike's `/tmp/wb-spike/…`)
   become `/work/project`; Claude's encoded project dir (`-private-tmp-wb-spike-claude`) becomes
   `-work-project`.
3. Any string over 2 KB is cut to its first 200 characters plus `…` (this truncates the user's
   hook output in `login-expired.jsonl` and the skill file Codex printed).
4. Claude `system/init`: `slash_commands`, `agents`, `tools`, `mcp_servers`, and also `skills`,
   `plugins`, `terminal_slash_commands` (the user's own configuration) are set to `[]`.

Lines were re-serialised as compact JSON; field order is preserved.
