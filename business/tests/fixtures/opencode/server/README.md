# Recorded `opencode serve` (OpenCode 1.19.3)

Real responses of `opencode serve --hostname 127.0.0.1 --port 0 --mdns=false`, recorded 2026-10-19 for the native chat
engine (issue #22, [plan](../../../../../docs/plans/opencode-native-chat.md)). Used as contract fixtures: the engine's
mapper and the HTTP/SSE client are tested against these bytes.

## How they were made

- `unshare -rn` (no network, loopback only), `env -i`, every XDG dir and `HOME` in a scratch folder, so no real config,
  credential or session was read or written.
- A scratch `opencode.json` with an OpenAI-compatible mock provider (`mock/m1`, `@ai-sdk/openai-compatible` is bundled)
  served by a scripted local mock model, a global MCP entry `probe` (`/bin/true`, so it fails to connect) and
  `share: disabled`. `OPENCODE_DISABLE_MODELS_FETCH=1`, `OPENCODE_DISABLE_AUTOUPDATE=1`, a random
  `OPENCODE_SERVER_PASSWORD`.
- The project was a fresh git repo. Its path is rewritten to `/work/project` (and the scratch root to `/work`); chunk
  sizes and `Content-Length` were re-encoded after the rewrite. The password is not in any file.

Sequence on one session: plain turn, `bash` ask allowed once, `write` ask rejected, `question` answered, steer while
busy, abort, image (`data:` URL) prompt, `!shell`, rename, fork at a message, clone, summarize, prompt with an unknown
model. `global-event.sse` is the raw `/global/event` stream of the whole run (chunked, ends without a final chunk
because the server was stopped).

## Files

| File | What |
| --- | --- |
| `health.http`, `health-unauthorized.http` | `GET /global/health` with and without Basic auth (401) |
| `global-event.sse` | raw `GET /global/event` response bytes |
| `config-providers-raw.http`, `messages-raw.http` | raw JSON responses (`Content-Length`; `messages` has `Link` / `x-next-cursor` paging) |
| `*.json` | `{request, status, body}` of each call (session create/get/patch/rename, agent, providers, command, mcp, permission list/reply/reject, question list/reply, prompt_async, steer, abort, shell, fork, summarize, status, messages) |
| `stderr.txt` | the server's stderr (empty) |

## Facts the recording confirmed

- The listening line arrived after 0.9 s. `--port 0` picked 4096 (OpenCode's default) when free: read the port from the
  line, never assume one.
- The server does **not** exit on stdin EOF. SIGTERM stops it.
- The global config's MCP servers load (`GET /mcp` shows `probe`).
- A prompt sent while busy is answered right after the running step. Abort ends the reply with `MessageAbortedError`
  and a `session.error`.
- An unknown model still gets `204` from `prompt_async`, then two `session.error` events (a short message, then one with
  a stack trace).
- `data:` image URLs are accepted. The question tool is on under `serve`.
- Durable events repeat as `payload.type == "sync"`.
