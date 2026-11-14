"""LocalDrop styled screens: the wireframe layouts (wf.py) restyled with colour variables,
real icons and Light / Dark modes, on their own page "Styled v1".

python styled.py tokens            create / update the colour variables (+ export tokens.json)
python styled.py system            design system sheet (light + dark)
python styled.py <key> [<key>...]  restyle wireframe screens (keys from wf.SCREENS)
python styled.py core              tokens + system + the core screens
python styled.py wire              re-link the styled prototype

The wireframes stay untouched: every styled screen is wf.SCREENS[key] passed through
`restyle()`, which swaps the wireframe greys for variables and the grey squares for icons.
"""
import copy
import json
import re
import sys

import fc
import wf

PAGE = "Styled v1"
COLLECTION = "LocalDrop"
MODES = ("Light", "Dark")

# ---------------------------------------------------------------- tokens
TOKENS = {  # name: (light, dark, use)
    "color/bg":          ("#F5F6F8", "#0F1114", "Window and screen background"),
    "color/surface":     ("#FFFFFF", "#181B20", "Cards, panes, sheets, inputs"),
    "color/surface-2":   ("#EEF0F3", "#22262D", "Selected rows, chips, icon tiles"),
    "color/line":        ("#E1E4E9", "#2C3139", "Borders and dividers"),
    "color/ink":         ("#14171C", "#E9EBEE", "Primary text"),
    "color/mute":        ("#5E6672", "#9BA3AE", "Secondary text, icons"),
    "color/faint":       ("#98A0AB", "#687180", "Placeholders, disabled, offline"),
    "color/accent":      ("#2563EB", "#60A5FA", "Primary buttons, active states, progress"),
    "color/on-accent":   ("#FFFFFF", "#0B1220", "Text and icons on accent"),
    "color/accent-soft": ("#EAF1FE", "#17243A", "Folder tiles, selected background"),
    "color/success":     ("#16A34A", "#4ADE80", "Online, done"),
    "color/warning":     ("#D97706", "#FBBF24", "Needs decision, low space"),
    "color/danger":      ("#DC2626", "#F87171", "Failed, destructive actions"),
}
# wireframe grey → token
WF_MAP = {wf.BG: "color/bg", wf.SURF: "color/surface", wf.LINE: "color/line",
          wf.BLOCK: "color/surface-2", wf.INK: "color/ink", wf.MUTE: "color/mute",
          wf.FAINT: "color/faint", wf.DARK: "color/accent"}
V = lambda name: {"$var": name}  # noqa: E731


def _cmd(command, params):
    return {"command": command, "params": params}


def tokens():
    have = json.loads(fc.call("run_command", {"command": "variable.list", "params": {}})
                      ["result"]["content"][0]["text"])
    names = {v["name"] for v in have.get("variables", [])}
    steps = []
    if not any(c["name"] == COLLECTION for c in have.get("collections", [])):
        steps.append(_cmd("variable.createCollection", {"name": COLLECTION,
                                                         "modes": list(MODES)}))
    for name, (light, dark, use) in TOKENS.items():
        if name in names:
            steps += [_cmd("variable.set", {"variable": name, "mode": "Light", "value": light}),
                      _cmd("variable.set", {"variable": name, "mode": "Dark", "value": dark})]
        else:
            steps.append(_cmd("variable.create", {"collection": COLLECTION, "name": name,
                                                   "type": "color", "description": use,
                                                   "values": {"Light": light, "Dark": dark}}))
    m = fc.call("batch", {"commands": steps, "label": "Styled: tokens"})
    if m.get("result", {}).get("isError"):
        raise RuntimeError(json.dumps(m)[:800])
    with open("../tokens.json", "w", encoding="utf-8") as f:
        json.dump({"collection": COLLECTION, "modes": list(MODES),
                   "colors": {k: {"light": v[0], "dark": v[1], "use": v[2]}
                              for k, v in TOKENS.items()}}, f, indent=2)
    print("tokens ok:", len(TOKENS))


# ---------------------------------------------------------------- icons (24 px grid)
def circle(cx, cy, r):
    k = 0.5523 * r
    return (f"M{cx - r} {cy} C{cx - r} {cy - k} {cx - k} {cy - r} {cx} {cy - r} "
            f"C{cx + k} {cy - r} {cx + r} {cy - k} {cx + r} {cy} "
            f"C{cx + r} {cy + k} {cx + k} {cy + r} {cx} {cy + r} "
            f"C{cx - k} {cy + r} {cx - r} {cy + k} {cx - r} {cy} Z")


DOT = lambda x, y: f"M{x} {y} L{x + 0.01} {y}"  # noqa: E731  (round cap → dot)

ICONS = {
    "folder": "M3 7.5 C3 6.1 3.9 5 5.2 5 L9.4 5 L11.4 7 L18.8 7 C20 7 21 8 21 9.2 L21 16.8 "
              "C21 18 20 19 18.8 19 L5.2 19 C3.9 19 3 18 3 16.8 Z",
    "file": "M6.5 3 L14 3 L19 8 L19 19.5 C19 20.3 18.3 21 17.5 21 L6.5 21 C5.7 21 5 20.3 5 "
            "19.5 L5 4.5 C5 3.7 5.7 3 6.5 3 Z M14 3 L14 8 L19 8",
    "image": "M5 4.5 L19 4.5 C19.8 4.5 20.5 5.2 20.5 6 L20.5 18 C20.5 18.8 19.8 19.5 19 19.5 "
             "L5 19.5 C4.2 19.5 3.5 18.8 3.5 18 L3.5 6 C3.5 5.2 4.2 4.5 5 4.5 Z "
             "M3.5 16 L8.5 11 L13 15.5 L16 12.5 L20.5 17 " + circle(15.5, 8.8, 1.6),
    "video": "M4.5 6 L14 6 C14.8 6 15.5 6.7 15.5 7.5 L15.5 16.5 C15.5 17.3 14.8 18 14 18 "
             "L4.5 18 C3.7 18 3 17.3 3 16.5 L3 7.5 C3 6.7 3.7 6 4.5 6 Z M15.5 10.5 L21 7.5 "
             "L21 16.5 L15.5 13.5",
    "music": "M9 17.5 L9 5.5 L20 3.5 L20 15.5 " + circle(6.5, 17.5, 2.5) + " " +
             circle(17.5, 15.5, 2.5),
    "inbox": "M12 3 L12 13 M8 9 L12 13 L16 9 M3.5 14 L8 14 L9.5 16.5 L14.5 16.5 L16 14 "
             "L20.5 14 L20.5 19.5 L3.5 19.5 Z",
    "drive": "M3 14 L21 14 L21 19 L3 19 Z M3 14 L6 5 L18 5 L21 14 " + DOT(17, 16.5),
    "share": circle(18, 5.5, 2.5) + " " + circle(6, 12, 2.5) + " " + circle(18, 18.5, 2.5) +
             " M8.2 10.8 L15.8 6.7 M8.2 13.2 L15.8 17.3",
    "transfers": "M7.5 4 L7.5 20 M3.5 8 L7.5 4 L11.5 8 M16.5 20 L16.5 4 M12.5 16 L16.5 20 "
                 "L20.5 16",
    "sliders": "M4 6 L8 6 M12 6 L20 6 M4 12 L13 12 M17 12 L20 12 M4 18 L6 18 M10 18 L20 18 " +
               circle(10, 6, 2) + " " + circle(15, 12, 2) + " " + circle(8, 18, 2),
    "copy": "M9 9 L19.5 9 C19.8 9 20 9.2 20 9.5 L20 19.5 C20 19.8 19.8 20 19.5 20 L9.5 20 "
            "C9.2 20 9 19.8 9 19.5 Z M5.5 15 L4.5 15 C4.2 15 4 14.8 4 14.5 L4 4.5 C4 4.2 4.2 4 "
            "4.5 4 L14.5 4 C14.8 4 15 4.2 15 4.5 L15 5.5",
    "arrow-right": "M5 12 L19 12 M13 6 L19 12 L13 18",
    "arrow-left": "M19 12 L5 12 M11 6 L5 12 L11 18",
    "trash": "M4 7 L20 7 M9.5 7 L9.5 4.5 L14.5 4.5 L14.5 7 M6.5 7 L7.5 20 L16.5 20 L17.5 7 "
             "M10 11 L10 16 M14 11 L14 16",
    "pencil": "M4 20 L8.5 19 L19 8.5 L15.5 5 L5 15.5 Z M13.5 7 L17 10.5",
    "more": DOT(6, 12) + " " + DOT(12, 12) + " " + DOT(18, 12),
    "send": "M21 3 L10 14 M21 3 L14.5 21 L10 14 L3 9.5 Z",
    "search": circle(11, 11, 6.5) + " M16 16 L20.5 20.5",
    "sort": "M7 4 L7 20 M3.5 16.5 L7 20 L10.5 16.5 M13.5 6 L20.5 6 M13.5 11 L18.5 11 "
            "M13.5 16 L16.5 16",
    "filter": "M4 5 L20 5 L14 12 L14 19 L10 17 L10 12 Z",
    "info": circle(12, 12, 9) + " M12 11 L12 16.5 " + DOT(12, 7.8),
    "alert": "M12 3.5 L21.5 20 L2.5 20 Z M12 10 L12 14 " + DOT(12, 17),
    "upload": "M12 15.5 L12 4 M7 9 L12 4 L17 9 M4 15.5 L4 20 L20 20 L20 15.5",
    "download": "M12 4 L12 15.5 M7 10.5 L12 15.5 L17 10.5 M4 15.5 L4 20 L20 20 L20 15.5",
    "laptop": "M5.5 5.5 L18.5 5.5 C19 5.5 19.5 6 19.5 6.5 L19.5 15.5 L4.5 15.5 L4.5 6.5 "
              "C4.5 6 5 5.5 5.5 5.5 Z M2.5 18.5 L21.5 18.5",
    "monitor": "M4 4.5 L20 4.5 C20.6 4.5 21 4.9 21 5.5 L21 15 C21 15.6 20.6 16 20 16 L4 16 "
               "C3.4 16 3 15.6 3 15 L3 5.5 C3 4.9 3.4 4.5 4 4.5 Z M8.5 20 L15.5 20 M12 16 L12 20",
    "tablet": "M7 3 L17 3 C17.8 3 18.5 3.7 18.5 4.5 L18.5 19.5 C18.5 20.3 17.8 21 17 21 L7 21 "
              "C6.2 21 5.5 20.3 5.5 19.5 L5.5 4.5 C5.5 3.7 6.2 3 7 3 Z M11 18 L13 18",
    "phone": "M8.5 3 L15.5 3 C16.3 3 17 3.7 17 4.5 L17 19.5 C17 20.3 16.3 21 15.5 21 L8.5 21 "
             "C7.7 21 7 20.3 7 19.5 L7 4.5 C7 3.7 7.7 3 8.5 3 Z M11 18 L13 18",
    "columns": "M4.5 4.5 L19.5 4.5 C20.1 4.5 20.5 4.9 20.5 5.5 L20.5 18.5 C20.5 19.1 20.1 "
               "19.5 19.5 19.5 L4.5 19.5 C3.9 19.5 3.5 19.1 3.5 18.5 L3.5 5.5 C3.5 4.9 3.9 4.5 "
               "4.5 4.5 Z M12 4.5 L12 19.5",
    "pause": "M8.5 5 L8.5 19 M15.5 5 L15.5 19",
    "wifi": "M2.5 9 C7.8 4.3 16.2 4.3 21.5 9 M5.5 12.5 C9.2 9.3 14.8 9.3 18.5 12.5 "
            "M8.8 16 C10.6 14.5 13.4 14.5 15.2 16 " + DOT(12, 19.5),
    "lock": "M6.5 11 L17.5 11 C18 11 18.5 11.5 18.5 12 L18.5 19.5 C18.5 20 18 20.5 17.5 20.5 "
            "L6.5 20.5 C6 20.5 5.5 20 5.5 19.5 L5.5 12 C5.5 11.5 6 11 6.5 11 Z "
            "M8.5 11 L8.5 7.5 C8.5 3.5 15.5 3.5 15.5 7.5 L15.5 11",
    "clock": circle(12, 12, 9) + " M12 7 L12 12 L15.5 14",
    "ethernet": "M6 4 L18 4 L18 13 L6 13 Z M9 13 L9 16.5 M15 13 L15 16.5 M7.5 16.5 L16.5 16.5 "
                "M12 16.5 L12 20.5 M9.5 7.5 L9.5 9.5 M12 7.5 L12 9.5 M14.5 7.5 L14.5 9.5",
    "plus": "M12 5 L12 19 M5 12 L19 12",
    "check": "M5 12.5 L10 17.5 L19 7",
    "x": "M6 6 L18 18 M18 6 L6 18",
    "logo": "M12 3.5 L12 13 M8 9 L12 13 L16 9 M4.5 13.5 L4.5 19.5 L19.5 19.5 L19.5 13.5",
}

_NUM = re.compile(r"-?\d*\.?\d+")


def scale_path(d, k, ox, oy):
    nums = [float(x) for x in _NUM.findall(d)]
    out, i = [], 0

    def sub(m):
        nonlocal i
        v = nums[i] * k + (ox if i % 2 == 0 else oy)
        i += 1
        return f"{v:.2f}"
    # every command used here (M L C) takes x,y pairs, so even = x, odd = y
    return _NUM.sub(sub, d)


def glyph(kind, size, color, sw=None):
    """A stroke icon as a vector, already scaled into a size×size box at (0, 0)."""
    k = size / 24
    return {"type": "vector", "name": f"Glyph / {kind}", "path": scale_path(ICONS[kind], k, 0, 0),
            "xy": (0, 0), "kids": [],
            "props": {"fills": [], "stroke": V(color), "strokeWeight": sw or max(1.25, 1.75 * k),
                      "strokeCap": "round", "strokeJoin": "round"}}


def icon_box(kind, size, radius=0, fill=None, color="color/mute", glyph_frac=None):
    """Fixed-size frame holding a centred glyph; with `fill` it is a tile."""
    g = round(size * (glyph_frac or (0.56 if fill else 1.0)))
    gl = glyph(kind, g, color)
    gl["xy"] = ((size - g) / 2, (size - g) / 2)
    props = {"fills": [], "cornerRadius": radius, "clip": False}
    if fill:
        props.pop("fills")
        props["fill"] = V(fill)
    return {"type": "frame", "name": f"Icon / {kind}", "w": size, "h": size,
            "props": {**props, "sizingH": "fixed", "sizingV": "fixed"}, "kids": [gl]}


# ---------------------------------------------------------------- picking icons from context
CTX = re.compile(r"^(Row|Tab|Cmd|Nav|Job|File|Hist|Target|Segment|Perm|Item|Toast|Note|Set|"
                 r"Done|Action|Option|Panel tab|Conflict) / (.+)$")
CTX_NAMES = {"Device head", "Device", "From", "Offline note", "Drop hint", "Background hint",
             "Transfer bar", "Space", "Note", "Mode", "Banner", "Hint", "Add hint", "Found",
             "Key warning", "Fat", "Brand", "Drop zone", "Search", "Info", "Shared icon",
             "Inbox icon", "Menu", "Files", "Offline", "Queue", "Header", "Looking", "Tray",
             "LocalDrop tray icon", "Discovery off", "Hero", "Start", "Sub", "Empty"}
# used only when no stronger context is around (a job's "Head" must not hide its "Job / …")
WEAK_CTX = {"Head", "Field", "Search bar", "Sheet", "Paused", "Radar", "Hero", "Body", "Main"}
IMG, VID, AUD = (".jpg", ".jpeg", ".png", ".heic"), (".mp4", ".mov"), (".mp3", ".flac")
DOC = (".pdf", ".docx", ".txt", ".md", ".zip", ".iso", ".mp")


def texts(node):
    out = []
    if node["type"] == "text":
        out.append(node["text"])
    for k in node["kids"]:
        out += texts(k)
    return out


def device_kind(s):
    s = s.lower()
    for key, kind in (("macbook", "laptop"), ("laptop", "laptop"), ("desk pc", "monitor"),
                      ("this pc", "monitor"), ("ipad", "tablet"), ("pixel", "phone"),
                      ("this phone", "phone")):
        if key in s:
            return kind
    return None


def pick(ctx_name, title, label, size):
    """Icon kind for a grey placeholder square, from the layer names and texts around it."""
    cat, _, rest = ctx_name.partition(" / ")
    lab = (rest or title or "").lower()
    t = (title or "").lower()
    if cat in ("Tab",):
        return {"files": "folder", "transfers": "transfers", "folders": "share",
                "settings": "sliders"}.get(lab, "folder")
    if cat == "Cmd":
        for key, kind in (("copy", "copy"), ("move ← ", "arrow-left"), ("move", "arrow-right"),
                          ("delete", "trash"), ("rename", "pencil"), ("send", "send"),
                          ("show", "folder"), ("more", "more")):
            if lab.startswith(key) or key in lab:
                return kind
    if cat == "Action":
        return {"search": "search", "sort": "sort", "filter": "filter", "rename": "pencil",
                "copy": "copy"}.get(lab, "more")
    if cat == "Segment":
        return {"remote": "monitor", "split": "columns", "all": "transfers",
                "outgoing": "upload", "incoming": "download"}.get(lab, "folder")
    if cat == "Nav":
        if lab == "transfers":
            return "transfers"
        if lab == "settings":
            return "sliders"
        if lab.startswith("searching"):
            return "wifi"
        return "folder"
    if ctx_name == "Empty":
        return "transfers" if "being sent" in lab else "share"
    if cat == "Item":
        lab = lab.removeprefix("danger ")
        for key, kind in (("pause", "pause"), ("details", "info"), ("folder", "folder"),
                          ("cancel", "x"), ("remove", "trash"), ("move", "upload")):
            if lab.startswith(key) or key in lab:
                return kind
    name = ctx_name.lower()
    if ctx_name == "Transfer bar":
        return "upload"
    if ctx_name in ("Brand", "Hero", "Tray", "LocalDrop tray icon"):
        return "logo"
    if cat == "Conflict":
        return "file"
    if ctx_name == "Head" and "localdrop" in label:
        return "logo"
    if cat == "Row" and any(k in label for k in ("wi-fi", "virtual", "hotspot")):
        return "wifi"
    if cat == "Row" and "ethernet" in label:
        return "ethernet"
    if ctx_name == "Paused":
        return "pause"
    if ctx_name in ("Search", "Field", "Search bar"):
        return "search"
    if ctx_name == "Head" and "key" in label:
        return "lock"
    if any(k in label for k in ("network", "discovery", "searching", "office-5g")):
        return "wifi"
    if ctx_name == "Head" and "localdrop" in label:
        return "logo"
    if ctx_name == "Head" and any(k in label for k in (" set", "backup")):
        return "folder"
    if ctx_name in ("Space", "Banner", "Key warning", "Fat") or "out of space" in label:
        return "alert"
    if t.endswith(IMG):
        return "image"
    if t.endswith(VID):
        return "video"
    if t.endswith(AUD) or t == "music":
        return "music"
    if t.endswith(DOC) or ".pdf" in t or "backup.zip" in t:
        return "file"
    dk = device_kind(t)
    if dk and cat not in ("Job", "Hist"):
        return dk
    if "inbox" in t:
        return "inbox"
    if any(k in t for k in ("disk", "drive", "storage", "sd card", "usb", ":)")):
        return "drive"
    if "shared folders" in t:
        return "share"
    if ctx_name in ("Offline note", "Drop hint", "Background hint", "Note", "Mode", "Hint",
                    "Add hint", "Found", "Info", "Files", "Menu", "Sub", "Discovery off"):
        return "info"
    if ctx_name in ("Drop zone",):
        return "upload"
    if cat in ("Job", "Hist", "Done", "Set", "Target", "Row", "File", "Perm", "Option",
               "Toast", "Note", "Item") or name in ("head", "header", "queue"):
        if cat == "Perm":
            return {"nearby devices": "wifi", "notifications": "info",
                    "run in background": "clock", "wi-fi name": "wifi"}.get(lab, "info")
        if cat == "Toast" or cat == "Note":
            return "logo"
        return "folder"
    return None


# ---------------------------------------------------------------- restyle a wireframe tree
def _var(c):
    return V(WF_MAP[c]) if isinstance(c, str) and c in WF_MAP else None


def restyle(node, ctx=("", "", ""), bg=None, unmatched=None):
    """Return a styled copy of a wireframe node tree."""
    n = copy.copy(node)
    p = dict(n["props"])
    n["props"] = p
    name = n["name"]

    # context for icon picking: nearest named row / block, its first text and all texts
    if CTX.match(name) or name in CTX_NAMES or (name in WEAK_CTX and not ctx[0]):
        tx = texts(node)
        ctx = (name, tx[0] if tx else "", " ".join(tx).lower())

    # colours → variables
    fills = p.get("fills")
    own_bg = bg
    if isinstance(fills, list) and len(fills) == 1 and _var(fills[0]):
        p.pop("fills")
        p["fill"] = _var(fills[0])
        own_bg = fills[0]
    strokes = p.get("strokes")
    if isinstance(strokes, list) and len(strokes) == 1 and _var(strokes[0]):
        p.pop("strokes")
        p["stroke"] = _var(strokes[0])
    if n["type"] in ("text", "rectangle") and _var(p.get("fill")):
        c = p["fill"]
        p["fill"] = _var(c)
        if n["type"] == "text" and c == wf.SURF and bg == wf.DARK:
            p["fill"] = V("color/on-accent")
    if n["type"] == "vector" and isinstance(strokes, list) and strokes and strokes[0] == wf.SURF \
            and bg == wf.DARK:
        p.pop("stroke", None)
        p["stroke"] = V("color/on-accent")

    # warning banners: outlined in the warning colour, not the accent
    if n["type"] == "frame" and name in ("Space", "Banner", "Key warning") and             node["props"].get("strokes") == [wf.DARK]:
        p["stroke"] = V("color/warning")

    # destructive menu entries → danger text
    if n["type"] == "text" and ctx[0].startswith("Item / Danger ") and ctx[1] == n.get("text"):
        p["fill"] = V("color/danger")

    # destructive primary buttons → danger
    if n["type"] == "frame" and name.startswith("Button / ") and             node["props"].get("fills") == [wf.DARK] and             re.match(r"Button / (Delete|Leave|Move to Trash|Remove|Forget|Clear all)", name):
        p["fill"] = V("color/danger")
        n["kids"] = [restyle(k, ctx, wf.DARK, unmatched) for k in node["kids"]]
        return n

    # status dots
    if n["type"] == "rectangle" and name == "Balance":
        p["fill"] = "#00000000"
    if n["type"] == "rectangle" and name == "Online":
        p["fill"] = V("color/success")

    # avatar circle → device tile
    if n["type"] == "rectangle" and name == "Block" and n.get("w") == n.get("h") and \
            (n.get("w") or 0) >= 28 and p.get("cornerRadius", 0) * 2 >= n["w"]:
        kind = device_kind(ctx[2]) or "laptop"
        return icon_box(kind, n["w"], radius=n["w"] / 2, fill="color/accent-soft",
                        color="color/accent")

    # grey placeholder squares → icons
    if n["type"] == "rectangle" and name == "Copy":
        return icon_box("copy", n.get("w") or 24, color="color/mute", glyph_frac=0.7)
    if n["type"] == "rectangle" and name == "Icon" and ctx[0] in ("Tray", "Taskbar") and             node["props"].get("fill") == wf.FAINT:
        p["fill"] = V("color/faint")
        n["kids"] = []
        return n
    if n["type"] == "rectangle" and (name == "Icon" or name.startswith("Action / ") or
                                     name in ("Logo",)):
        size = n.get("w") or 24
        col = node["props"].get("fill")
        cname = name if name.startswith("Action / ") else ctx[0]
        kind = pick(cname, ctx[1], ctx[2], size)
        if name == "Logo" or (kind == "logo"):
            return icon_box("logo", size, radius=p.get("cornerRadius", size / 4),
                            fill="color/accent", color="color/on-accent", glyph_frac=0.62)
        if kind == "alert":
            return icon_box("alert", size, color="color/warning")
        if kind is None:
            if unmatched is not None:
                unmatched.add(f"{ctx[0]!r} / {ctx[1]!r} ({size})")
            kind = "file" if size >= 28 else "info"
        cat = cname.partition(" / ")[0]
        folderish = kind in ("folder", "share", "inbox", "drive")
        if cat in ("Tab", "Cmd", "Nav", "Segment", "Action"):
            # navigation / command icons: bare glyph, accent when active
            ink = ("color/on-accent" if bg == wf.DARK else
                   "color/accent" if col == wf.DARK else "color/mute")
            return icon_box(kind, size, color=ink)
        if size <= 24 and not p.get("cornerRadius", 0) > size / 3 and col != wf.DARK:
            # small icon: bare glyph
            return icon_box(kind, size, color="color/accent" if folderish else "color/mute")
        if col == wf.DARK:
            return icon_box(kind, size, radius=p.get("cornerRadius", 8), fill="color/accent",
                            color="color/on-accent")
        tile = "color/accent-soft" if kind in ("folder", "share", "inbox", "drive") else \
            "color/surface-2"
        ink = "color/accent" if tile == "color/accent-soft" else "color/mute"
        return icon_box(kind, size, radius=p.get("cornerRadius", 8), fill=tile, color=ink)

    n["kids"] = [restyle(k, ctx, own_bg, unmatched) for k in node["kids"]]
    return n


# ---------------------------------------------------------------- build
def emit(node, parent, steps, xy=None):
    ref = f"n{len(steps)}"
    params = {"type": node["type"], "name": node["name"], "props": dict(node["props"]),
              "select": False}
    if node.get("link"):
        params["props"]["pluginData"] = {"link": node["link"]}
    for key, dst in (("text", "text"), ("path", "path"), ("w", "width"), ("h", "height")):
        if node.get(key) is not None:
            params[dst] = node[key]
    if parent:
        params["parent"] = {"$ref": parent}
    pos = xy or node.get("xy")
    if pos:
        params["x"], params["y"] = pos
    steps.append({"command": "node.create", "as": ref, "params": params})
    for k in node["kids"]:
        emit(k, ref, steps)
    return ref


def _json(m):
    if "error" in m or m.get("result", {}).get("isError"):
        raise RuntimeError(json.dumps(m)[:1500])
    return json.loads(m["result"]["content"][0]["text"])


def goto_page():
    pages = _json(fc.call("run_command", {"command": "page.list", "params": {}}))
    pg = next((p for p in pages if p["name"] == PAGE), None)
    if not pg:
        pid = _json(fc.call("run_command", {"command": "page.add", "params": {"name": PAGE}}))
        pid = pid.get("id") if isinstance(pid, dict) else pid
        pages = _json(fc.call("run_command", {"command": "page.list", "params": {}}))
        pg = next(p for p in pages if p["name"] == PAGE)
    fc.call("run_command", {"command": "page.select", "params": {"id": pg["id"]}})
    return pg["id"]


def back_to_wireframes():
    pages = _json(fc.call("run_command", {"command": "page.list", "params": {}}))
    fc.call("run_command", {"command": "page.select", "params": {"id": pages[0]["id"]}})


def page_frames():
    doc = _json(fc.call("inspect_document", {"depth": 1, "fields": ["id", "name", "flows"]}))
    page = next(p for p in doc["pages"] if p["id"] == doc["currentPage"])
    return page


def place(tree, xy, mode, unmatched):
    """Create one styled frame (deleting an older one with the same name) in a mode."""
    old = [c["id"] for c in page_frames().get("children", []) if c["name"] == tree["name"]]
    if old:
        fc.call("run_command", {"command": "edit.delete", "params": {"ids": old}})
    steps = []
    emit(tree, None, steps, xy)
    root = _json(fc.call("batch", {"commands": steps, "label": f"Styled: {tree['name']}"}))[0]["id"]
    fc.call("run_command", {"command": "variable.setMode",
                            "params": {"ids": [root], "collection": COLLECTION, "mode": mode}})
    return root


CORE = ("d-remote", "d-split", "d-transfers", "files-remote", "transfers", "send")
ROW_Y = {"system": 0, "desk-light": 1200, "desk-dark": 2100, "phone-light": 3100,
         "phone-dark": 4100}


def screen(key):
    fn, slot = wf.SCREENS[key]
    unmatched = set()
    base = restyle(fn(), unmatched=unmatched)
    desk = slot >= 99
    order = list(wf.SCREENS).index(key)
    keys = [k for k in CORE if (wf.SCREENS[k][1] >= 99) == desk] +         [k for k in wf.SCREENS if k not in CORE and (wf.SCREENS[k][1] >= 99) == desk]
    i = keys.index(key)
    w = (wf.DESK[0] if desk else wf.PHONE[0]) + wf.GAP_X
    for mode in MODES:
        t = copy.deepcopy(base)
        t["name"] = f"{'Desktop' if desk else 'Phone'} / {key} · {mode}"
        t["props"]["pluginData"] = {"screen": key, "mode": mode}
        row = f"{'desk' if desk else 'phone'}-{mode.lower()}"
        rid = place(t, (i * w, ROW_Y[row]), mode, unmatched)
        print(key, mode, "ok", rid)
    if unmatched:
        print("  unmatched icons:", *sorted(unmatched), sep="\n   ")
    return order


# ---------------------------------------------------------------- design system sheet
T, F, R, row, col = wf.T, wf.F, wf.R, wf.row, wf.col


def swatch(name, light, dark, use):
    chip_ = F("Swatch", w=56, h=56, sizingH="fixed", sizingV="fixed", cornerRadius=12,
              fills=[], strokes=[wf.LINE], strokeWeight=1)
    chip_["props"]["fill"] = V(name)
    chip_["props"].pop("fills")
    return row(f"Token / {name}", chip_,
               col("Text", T(name, 13, "Semi Bold"), T(use, 12, color=wf.MUTE),
                   T(f"{light}  ·  {dark}", 11, color=wf.FAINT), gap=2), gap=12)


def system_sheet():
    W = 1600
    colors = col("Colours", T("Colour", 20, "Bold"),
                 T("One collection “LocalDrop”, modes Light and Dark. Screens pick a mode; "
                   "every layer is bound to a variable.", 13, color=wf.MUTE, wrap=True),
                 *[swatch(k, *v) for k, v in TOKENS.items()], gap=12, sizingH="fixed", w=420)
    type_ = col("Type", T("Type · Inter", 20, "Bold"),
                *[row(f"Type / {lbl}", T(lbl, 12, color=wf.MUTE, name="Spec"),
                      T(sample, size, weight), gap=16, align="center")
                  for lbl, size, weight, sample in (
                      ("Title 24 Bold", 24, "Bold", "Dmytro's Laptop"),
                      ("Heading 20 Bold", 20, "Bold", "Send 3 items"),
                      ("Subhead 15 Semi Bold", 15, "Semi Bold", "Waiting to send · 3"),
                      ("Body 14 Medium", 14, "Medium", "Holiday 2024"),
                      ("Body 13 Regular", 13, "Regular", "1 820 files · 3.1 GB"),
                      ("Caption 12", 12, "Regular", "Queued 2 h ago"),
                      ("Label 11 Semi Bold", 11, "Semi Bold", "SHARED WITH YOUR GROUP"))],
                T("Radii: 6 icons · 8 inputs, small buttons · 10 buttons · 12–14 cards · "
                  "18–20 dialogs, sheets", 12, color=wf.MUTE, wrap=True),
                T("Spacing: 4-pt grid; 16 phone gutter, 32 desktop gutter", 12, color=wf.MUTE),
                gap=14, sizingH="fixed", w=400)
    comps = col("Components", T("Components", 20, "Bold"),
                row("Buttons", wf.button("Send"), wf.button("Cancel", primary=False),
                    wf.button("Download", small=True),
                    wf.button("Rename", primary=False, small=True), gap=10),
                row("Chips", wf.chip("All", True), wf.chip("Photos"), wf.chip("2 queued"),
                    wf.chip("Needs decision", True), gap=8),
                row("Controls", wf.toggle(True), wf.toggle(False), wf.checkbox(True),
                    wf.checkbox(False), wf.radio(True), wf.radio(False), wf.small_toggle(True),
                    gap=14),
                wf.field("Group name", "home"),
                row("Pills", wf.disc_pill(), wf.seg("Remote", "Split", active=0, links={}),
                    gap=12),
                col("Progress", wf.progress(0.34), wf.progress(0.72), gap=8),
                wf.card(wf.frow("Photos", ("2 184 items",), (90,), checked=False,
                                sub="Pictures", trailing="›"),
                        wf.divider(48),
                        wf.frow("IMG_0001.jpg", ("4.1 MB",), (90,), checked=True),
                        wf.divider(48),
                        wf.frow("VID_0003.mp4", ("812 MB",), (90,), checked=False)),
                row("Side", col("Sidebar", wf.side_item("Transfers", True,
                                                         trailing=wf.chip("2", True)),
                                wf.side_item("Settings"), gap=4, sizingH="fixed", w=240),
                    wf.transfer_bar(), gap=16, align="min"),
                gap=16, sizingH="fixed", w=560)
    icons = col("Icons", T("Icons · 24 px grid, 1.75 stroke", 20, "Bold"),
                *[row(f"Grid {r}", *[col(f"Icon cell / {k}", icon_box(k, 24, color="color/ink"),
                                         T(k, 10, color=wf.MUTE), gap=6, crossAlign="center",
                                         sizingH="fixed", w=84)
                                     for k in list(ICONS)[r * 16:(r + 1) * 16]], gap=8)
                  for r in range((len(ICONS) + 15) // 16)],
                gap=14)
    head = col("Head", T("LocalDrop · design system", 28, "Bold"),
               T("Calm utility · blue accent · Inter · light and dark", 14, color=wf.MUTE),
               gap=4)
    body = F("Design system", head, row("Columns", colors, type_, comps, gap=48, align="min"),
             icons, w=W, layout="vertical", gap=40, padding=48, sizingV="hug",
             fills=[wf.BG])
    return body


def system():
    unmatched = set()
    base = restyle(system_sheet(), unmatched=unmatched)
    for i, mode in enumerate(MODES):
        t = copy.deepcopy(base)
        t["name"] = f"Design system · {mode}"
        rid = place(t, (i * 1720, ROW_Y["system"]), mode, unmatched)
        print("system", mode, "ok", rid)


# ---------------------------------------------------------------- prototype
def wire():
    page = page_frames()
    frames = {}
    for c in page.get("children", []):
        m = re.match(r"^(?:Desktop|Phone) / (.+) · (Light|Dark)$", c["name"])
        if m:
            frames[(m.group(1), m.group(2))] = c["id"]
    links = []
    for (key, mode), fid in frames.items():
        node = _json(fc.call("get_node", {"id": fid, "depth": 60, "fields": ["pluginData"],
                                          "maxBytes": 4000000}))
        found = []
        wf._links(node, found)
        links += [(nid, t, mode) for nid, t in found]
    steps = [_cmd("proto.interaction.clear", {"ids": [i for i, _, _ in links]})]
    for fl in page.get("flows") or []:
        start = fl.get("start") or fl.get("startingPoint") or fl.get("id")
        if start:
            steps.append(_cmd("proto.flow.remove", {"start": start}))
    flows = [("Styled · Desktop", "d-remote", "Light"),
             ("Styled · Desktop dark", "d-remote", "Dark"),
             ("Styled · Phone", "files-remote", "Light"),
             ("Styled · Phone dark", "files-remote", "Dark")]
    # event-driven states keep their own entry, as on the wireframe page (light only)
    flows += [(f"Styled · {name}", key, "Light") for name, key in wf.FLOWS.items()
              if name not in ("Desktop", "Phone")]
    flows.append(("Styled · First run", "d-join", "Light"))
    flows.append(("Styled · Phone first run", "join", "Light"))
    for name, key, mode in flows:
        if (key, mode) in frames:
            steps.append(_cmd("proto.flow.add", {"start": frames[(key, mode)], "name": name}))
    n = 0
    for nid, target, mode in links:
        if target == "back":
            p = {"id": nid, "trigger": {"type": "onClick"}, "action": {"type": "back"}}
        elif (target, mode) in frames:
            p = {"id": nid, "trigger": {"type": "onClick"},
                 "action": {"type": "navigate", "dest": frames[(target, mode)]},
                 "transition": {"type": "dissolve"}, "durationMs": 150, "easing": "easeOut"}
        else:
            continue
        steps.append(_cmd("proto.interaction.add", p))
        n += 1
    _json(fc.call("batch", {"commands": steps, "label": "Styled: prototype links"}))
    print(f"styled: wired {n} links on {len(frames)} frames (links to unstyled screens skipped)")


if __name__ == "__main__":
    args = sys.argv[1:]
    goto_page()
    try:
        if args == ["tokens"]:
            tokens()
        elif args == ["wire"]:
            wire()
        else:
            todo = []
            for a in args:
                todo += ["tokens", "system", *CORE] if a == "core" else [a]
            for a in todo:
                if a == "tokens":
                    tokens()
                elif a == "system":
                    system()
                else:
                    screen(a)
            wire()
    finally:
        back_to_wireframes()
