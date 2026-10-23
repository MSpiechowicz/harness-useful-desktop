"""S0: record real `codex app-server` traffic for the Codex harness plan.

Usage: python3 -I record.py <project_dir> <out_dir>
Every frame in both directions lands in <out_dir>/<scenario>.jsonl as {"dir": "in"|"out", "frame": …}.
"""
import json
import os
import subprocess
import sys
import threading
import time
import queue

PROJECT, OUT = sys.argv[1], sys.argv[2]
# The configured gpt-6-astra is refused for a ChatGPT login: use model/list's default.
MODEL = "gpt-5.6-terra"
os.makedirs(OUT, exist_ok=True)

proc = subprocess.Popen(
    ["codex", "app-server", "-c", "check_for_update_on_startup=false"],
    cwd=PROJECT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=open(os.path.join(OUT, "stderr.log"), "w"),
    text=True, bufsize=1,
)
inbox: "queue.Queue[dict]" = queue.Queue()
log = None
next_id = 0
# Decision for each server request method, changed per scenario.
decisions: dict = {}


def reader():
    for line in proc.stdout:
        line = line.strip()
        if not line:
            continue
        try:
            frame = json.loads(line)
        except json.JSONDecodeError:
            frame = {"_raw": line}
        inbox.put(frame)
    inbox.put({"_eof": True})


threading.Thread(target=reader, daemon=True).start()


def write(frame):
    if log:
        log.write(json.dumps({"dir": "out", "t": time.time(), "frame": frame}) + "\n")
        log.flush()
    proc.stdin.write(json.dumps(frame) + "\n")
    proc.stdin.flush()


def scenario(name):
    global log
    if log:
        log.close()
    log = open(os.path.join(OUT, f"{name}.jsonl"), "w")
    print(f"== {name}", flush=True)


def handle(frame):
    """Log a frame, answer server requests. Returns the frame."""
    if log:
        log.write(json.dumps({"dir": "in", "t": time.time(), "frame": frame}) + "\n")
        log.flush()
    if "method" in frame and "id" in frame:
        method = frame["method"]
        answer = decisions.get(method)
        if callable(answer):
            answer = answer(frame)
        if answer is None:
            write({"id": frame["id"], "error": {"code": -32601, "message": "not supported by recorder"}})
        else:
            write({"id": frame["id"], "result": answer})
        print(f"   request {method} -> {json.dumps(answer)[:80]}", flush=True)
    return frame


def pump(until, timeout=180):
    """Read frames until `until(frame)` is true. Returns that frame."""
    end = time.time() + timeout
    while time.time() < end:
        try:
            frame = inbox.get(timeout=1)
        except queue.Empty:
            continue
        if frame.get("_eof"):
            raise SystemExit("app-server exited")
        handle(frame)
        if until(frame):
            return frame
    raise TimeoutError("timed out")


def request(method, params=None, timeout=120):
    global next_id
    next_id += 1
    rid = next_id
    frame = {"id": rid, "method": method}
    if params is not None:
        frame["params"] = params
    write(frame)
    resp = pump(lambda f: f.get("id") == rid and "method" not in f, timeout)
    if "error" in resp:
        print(f"   {method} ERROR {resp['error']}", flush=True)
    return resp


def notify(method, params=None):
    frame = {"method": method}
    if params is not None:
        frame["params"] = params
    write(frame)


def turn_done(thread_id):
    return lambda f: f.get("method") == "turn/completed" and f["params"].get("threadId") == thread_id


def text(t):
    return [{"type": "text", "text": t, "text_elements": []}]


def drain(seconds):
    end = time.time() + seconds
    while time.time() < end:
        try:
            handle(inbox.get(timeout=0.5))
        except queue.Empty:
            pass


# 1. Handshake and catalog reads.
scenario("01_handshake")
request("initialize", {"clientInfo": {"name": "harness_desktop", "title": "Harness Desktop", "version": "0.0.0-s0"},
                       "capabilities": {"experimentalApi": True, "requestAttestation": False}})
notify("initialized")
drain(2)
for m, p in [("account/read", {"refreshToken": False}), ("model/list", {}), ("skills/list", {"cwds": [PROJECT]}),
             ("hooks/list", {"cwds": [PROJECT]}), ("mcpServerStatus/list", {}), ("account/rateLimits/read", None)]:
    request(m, p)

# 2. Plain turn with reasoning, in Always ask (untrusted + workspace-write): file write and command approvals, accepted.
scenario("02_turn_approvals_accept")
decisions = {"item/commandExecution/requestApproval": {"decision": "accept"},
             "item/fileChange/requestApproval": {"decision": "accept"},
             "item/permissions/requestApproval": {"permissions": {}, "scope": "turn"}}
r = request("thread/start", {"cwd": PROJECT, "model": MODEL, "approvalPolicy": "untrusted", "sandbox": "workspace-write"})
tid = r["result"]["thread"]["id"]
request("turn/start", {"threadId": tid, "input": text(
    "Create a file hello.txt containing the single word hi, then run the shell command `ls -1` and tell me the output. Be brief.")})
pump(turn_done(tid))
first_turn = None
turns = request("thread/turns/list", {"threadId": tid, "sortDirection": "asc"})
try:
    first_turn = turns["result"]["data"][0]["id"]
except Exception:
    pass

# 3. Declined command.
scenario("03_turn_decline")
decisions["item/commandExecution/requestApproval"] = {"decision": "decline"}
request("turn/start", {"threadId": tid, "input": text("Run the shell command `cat hello.txt`. If it is not allowed, just say so.")})
pump(turn_done(tid))

# 4. Steer a running turn.
scenario("04_steer")
decisions["item/commandExecution/requestApproval"] = {"decision": "accept"}
r = request("turn/start", {"threadId": tid, "input": text("Run `sleep 6 && echo done` and report the result.")})
turn_id = r.get("result", {}).get("turn", {}).get("id")
pump(lambda f: f.get("method") == "item/started" and f["params"].get("item", {}).get("type") == "commandExecution")
request("turn/steer", {"threadId": tid, "expectedTurnId": turn_id, "input": text("Also mention the word STEERED in your answer.")})
pump(turn_done(tid))

# 5. Interrupt.
scenario("05_interrupt")
r = request("turn/start", {"threadId": tid, "input": text("Run `sleep 30 && echo late` and report the result.")})
turn_id = r.get("result", {}).get("turn", {}).get("id")
time.sleep(4)
request("turn/interrupt", {"threadId": tid, "turnId": turn_id})
pump(turn_done(tid))

# 6. Rename, shell command, compact.
scenario("06_ops")
request("thread/name/set", {"threadId": tid, "name": "S0 recording"})
request("thread/shellCommand", {"threadId": tid, "command": "echo from-shell"})
drain(5)
request("thread/compact/start", {"threadId": tid})
drain(25)

# 7. Fork at the first turn, clone, list/read.
scenario("07_fork_list")
if first_turn:
    request("thread/fork", {"threadId": tid, "lastTurnId": first_turn, "excludeTurns": True})
request("thread/fork", {"threadId": tid, "excludeTurns": True})
request("thread/list", {"cwd": PROJECT, "limit": 10})
request("thread/read", {"threadId": tid, "includeTurns": True})
request("thread/items/list", {"threadId": tid})

# 8. Ephemeral utility thread (AI naming) on the default model.
scenario("08_ephemeral_naming")
r = request("thread/start", {"cwd": PROJECT, "model": MODEL, "ephemeral": True, "approvalPolicy": "never", "sandbox": "read-only"})
eid = r["result"]["thread"]["id"]
request("turn/start", {"threadId": eid, "effort": "low", "input": text(
    "Give a 3-word title for a chat that starts with: 'create hello.txt and list files'. Reply with the title only.")})
pump(turn_done(eid))

# 9. Question dialog (experimental request_user_input).
scenario("09_user_input")
decisions["item/tool/requestUserInput"] = lambda f: {"answers": {q["id"]: {"answers": [q.get("options", [{}])[0].get("label", "yes") if q.get("options") else "blue"]} for q in f["params"].get("questions", [])}}
request("turn/start", {"threadId": tid, "input": text(
    "Use your request_user_input tool (if you have it) to ask me one multiple-choice question: my favourite colour, options red or blue. Then repeat my answer. If you do not have the tool, say NO_TOOL.")})
pump(turn_done(tid))

scenario("10_cleanup_list")
request("thread/list", {"cwd": PROJECT, "limit": 10})
proc.stdin.close()
proc.wait(timeout=20)
print("exit", proc.returncode)
