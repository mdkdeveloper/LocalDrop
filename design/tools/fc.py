"""Minimal MCP stdio client for figcraft-cli attached to the running FigCraft app.

python fc.py tools [name...]          list tools (full docs for the given names)
python fc.py call <tool> <json|@file> call a tool; image content is saved to shot.png
"""
import base64
import json
import os
import subprocess
import sys

CLI = r"C:\Program Files\ArtCraft\FigCraft\figcraft-cli.exe"
HERE = os.environ.get("FC_OUT") or os.path.join(os.path.dirname(os.path.abspath(__file__)), "out")
os.makedirs(HERE, exist_ok=True)


def session(msgs):
    p = subprocess.Popen([CLI, "mcp", "--attach", "47801", "--timeout", "300"],
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    init = {"jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                       "clientInfo": {"name": "claude-code", "version": "0"}}}
    lines = [init, {"jsonrpc": "2.0", "method": "notifications/initialized"}] + msgs
    out, _ = p.communicate("\n".join(json.dumps(m) for m in lines).encode() + b"\n", timeout=600)
    res = {}
    for line in out.decode("utf-8", "replace").splitlines():
        line = line.strip()
        if line.startswith("{"):
            m = json.loads(line)
            if "id" in m:
                res[m["id"]] = m
    return res


def call(tool, args):
    r = session([{"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                  "params": {"name": tool, "arguments": args}}])
    return r.get(1, {})


def main():
    if sys.argv[1] == "tools":
        tools = session([{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}])[1]["result"]["tools"]
        for t in tools:
            if len(sys.argv) > 2 and t["name"] not in sys.argv[2:]:
                continue
            print("##", t["name"])
            print(t["description"])
            print(json.dumps(t["inputSchema"].get("properties", {}), indent=1))
        return
    a = sys.argv[3] if len(sys.argv) > 3 else "{}"
    args = json.load(open(a[1:], encoding="utf-8")) if a.startswith("@") else json.loads(a)
    m = call(sys.argv[2], args)
    if "error" in m:
        print("ERROR", json.dumps(m["error"]))
        sys.exit(1)
    n = 0
    for c in m["result"].get("content", []):
        if c.get("type") == "text":
            print(c["text"])
        elif c.get("type") == "image":
            path = os.path.join(HERE, "shot.png" if n == 0 else f"shot{n}.png")
            with open(path, "wb") as f:
                f.write(base64.b64decode(c["data"]))
            print("[image saved:", path, "]")
            n += 1
    if m["result"].get("isError"):
        sys.exit(1)


if __name__ == "__main__":
    main()
