"""Scrub S0 recordings into fixtures: paths, account, long lists cut to three."""
import json, os, re, sys
src, dst, project = sys.argv[1], sys.argv[2], sys.argv[3]
SUBS = [
    (project, "/work/project"),
    ("/home/frogius/Projekty/track-things-backend", "/work/extra-root"),
    ("/home/frogius", "/home/user"),
    ("maciej.spiechowicz@gmail.com", "user@example.invalid"),
    ("frogius", "user"),
]
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
