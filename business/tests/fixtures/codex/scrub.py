"""Scrub S0 recordings into fixtures: paths, account, long lists cut to three.

Usage: SCRUB_EMAIL=<account email> SCRUB_EXTRA='<path>=<replacement>' python3 -I scrub.py <src> <dst> <project>
"""
import json, os, re, sys
src, dst, project = sys.argv[1], sys.argv[2], sys.argv[3]
# The recorder's own home, user name and account email never reach a fixture. Extra private
# paths (other writable roots, …) go in SCRUB_EXTRA as "path=replacement;…".
HOME = os.path.expanduser("~")
SUBS = [(project, "/work/project")]
for pair in filter(None, os.environ.get("SCRUB_EXTRA", "").split(";")):
    old, _, new = pair.partition("=")
    SUBS.append((old, new or "/work/extra-root"))
SUBS += [(HOME, "/home/user"), (os.path.basename(HOME), "user")]
if os.environ.get("SCRUB_EMAIL"):
    SUBS.append((os.environ["SCRUB_EMAIL"], "user@example.invalid"))
def cut(v):
    if isinstance(v, list):
        return [cut(x) for x in v[:3]]
    if isinstance(v, dict):
        return {k: cut(x) for k, x in v.items()}
    if isinstance(v, str):
        for a, b in SUBS:
            v = v.replace(a, b)
        return v
    return v
for name in sorted(os.listdir(src)):
    if not name.endswith(".jsonl"):
        continue
    with open(os.path.join(src, name)) as fi, open(os.path.join(dst, name), "w") as fo:
        for line in fi:
            r = json.loads(line)
            fo.write(json.dumps({"dir": r["dir"], "frame": cut(r["frame"])}, ensure_ascii=False) + "\n")
