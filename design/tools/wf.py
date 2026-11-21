"""LocalDrop wireframe generator for FigCraft (via fc.py MCP client).

Needs FigCraft running as: figcraft --control 47801 --automation-write-root <repo>/design

python wf.py <screen> [<screen> ...]   (re)build screens; an existing frame with the same
                                       name is deleted first (each screen = one undo step)
python wf.py all                       rebuild every screen
python wf.py list                      list screen names
python wf.py wire                      only re-link the prototype (also runs after every build)
"""
import json
import sys

import fc

# Wireframe greys (no brand colour on purpose).
BG, SURF, LINE, BLOCK = "#F4F4F5", "#FFFFFF", "#D4D4D8", "#E4E4E7"
INK, MUTE, FAINT, DARK = "#18181B", "#71717A", "#A1A1AA", "#3F3F46"

PHONE = (390, 844)
DESK = (1280, 800)
GAP_X = 120


# ---------------------------------------------------------------- primitives
def F(name, *kids, w=None, h=None, **p):
    p.setdefault("fills", [])
    return {"type": "frame", "name": name, "w": w, "h": h, "props": p, "kids": list(kids)}


def T(s, size=14, weight="Regular", color=INK, name=None, wrap=False, **p):
    props = {"fontFamily": "Inter", "fontSize": size, "fontStyle": weight, "fill": color, **p}
    if wrap:
        props.update(sizingH="fill", textAutoResize="height")
    return {"type": "text", "name": name or s[:32], "text": s, "props": props, "kids": []}


def R(w, h, color=BLOCK, radius=0, name="Block", **p):
    return {"type": "rectangle", "name": name, "w": w, "h": h,
            "props": {"fill": color, "cornerRadius": radius, **p}, "kids": []}


def row(name, *kids, gap=12, pad=0, align="center", **p):
    return F(name, *kids, layout="horizontal", gap=gap, padding=pad, crossAlign=align,
             sizingH=p.pop("sizingH", "fill"), sizingV=p.pop("sizingV", "hug"), **p)


def col(name, *kids, gap=8, pad=0, **p):
    return F(name, *kids, layout="vertical", gap=gap, padding=pad,
             sizingH=p.pop("sizingH", "fill"), sizingV=p.pop("sizingV", "hug"), **p)


def link(node, target):
    """Make a layer clickable in the prototype: target = screen key (see SCREENS) or "back"."""
    node["link"] = target
    return node


def spacer(name="Spacer"):
    return F(name, layout="horizontal", sizingH="fill", sizingV="hug", h=1)


# ---------------------------------------------------------------- components
def icon(size=24, radius=6, color=BLOCK, name="Icon"):
    return R(size, size, color, radius, name=name)


def button(label, primary=True, full=False, small=False):
    return row(f"Button / {label}", T(label, 13 if small else 15, "Semi Bold",
                                      SURF if primary else INK),
               gap=8, pad=[8, 14, 8, 14] if small else [12, 20, 12, 20], mainAlign="center",
               sizingH="fill" if full else "hug",
               fills=[DARK if primary else SURF], cornerRadius=10,
               strokes=[] if primary else [LINE], strokeWeight=1)


def chip(label, filled=False):
    return row(f"Chip / {label}", T(label, 12, "Medium", SURF if filled else INK),
               pad=[4, 10, 4, 10], sizingH="hug", fills=[DARK if filled else BLOCK],
               cornerRadius=999)


def field(label, value, hint=None, trailing=None):
    box = row("Input", T(value, 15, color=INK if value[:1] != "•" else INK),
              spacer(), *( [T(trailing, 13, "Medium", MUTE)] if trailing else []),
              pad=[12, 14, 12, 14], fills=[SURF], strokes=[LINE], strokeWeight=1,
              cornerRadius=10)
    kids = [T(label, 13, "Medium", MUTE), box]
    if hint:
        kids.append(T(hint, 12, color=MUTE, wrap=True))
    return col(f"Field / {label}", *kids, gap=6)


def progress(frac, w=None):
    total = w or 300
    return F("Progress", R(max(4, int(total * frac)), 6, DARK, 3, name="Value"),
             layout="horizontal", sizingH="fill", sizingV="fixed", h=6,
             fills=[BLOCK], cornerRadius=3, clip=True)


def section(title, trailing=None):
    kids = [T(title.upper(), 11, "Semi Bold", MUTE, letterSpacing=0.6), spacer()]
    if trailing:
        kids.append(T(trailing, 12, "Medium", MUTE))
    return row(f"Section / {title}", *kids, pad=[16, 20, 6, 20])


def list_row(title, sub=None, lead="icon", trailing=None, status=None, pad_x=20):
    texts = [T(title, 15, "Medium")]
    if sub:
        texts.append(T(sub, 13, color=MUTE))
    kids = []
    if lead == "icon":
        kids.append(icon(36, 10))
    elif lead == "dot-on":
        kids.append(F("Avatar", R(36, 36, BLOCK, 18), w=36, h=36))
    elif lead:
        kids.append(lead)
    kids.append(col("Text", *texts, gap=2))
    if status:
        kids.append(status)
    if trailing:
        kids.append(trailing if isinstance(trailing, dict) else T(trailing, 13, "Medium", MUTE))
    return row(f"Row / {title}", *kids, pad=[12, pad_x, 12, pad_x])


def divider(inset=20):
    return F("Divider", R(10, 1, LINE, name="Line", sizingH="fill"), layout="horizontal",
             padding=[0, 0, 0, inset], sizingH="fill", sizingV="hug")


def status_dot(online):
    return R(10, 10, DARK if online else FAINT, 5, name="Online" if online else "Offline")


def radio(selected):
    return F("Radio", *( [R(10, 10, DARK, 5, name="Dot")] if selected else []),
             layout="horizontal", mainAlign="center", crossAlign="center", w=20, h=20,
             sizingH="fixed", sizingV="fixed", cornerRadius=10,
             strokes=[DARK if selected else FAINT], strokeWeight=2)


def tick(color=SURF, size=10):
    return {"type": "vector", "name": "Tick", "path": "M0 5 L3.6 8.6 L10 1.6",
            "w": size, "h": size * 0.7,
            "props": {"fills": [], "strokes": [color], "strokeWeight": 2}, "kids": []}


def checkbox(checked):
    return F("Checkbox", *( [tick()] if checked else []),
             layout="horizontal", mainAlign="center", crossAlign="center", w=20, h=20,
             sizingH="fixed", sizingV="fixed", cornerRadius=5,
             fills=[DARK] if checked else [], strokes=[DARK if checked else FAINT],
             strokeWeight=2)


def toggle(on):
    knob = R(18, 18, SURF, 9, name="Knob")
    return F("Toggle", knob, layout="horizontal", mainAlign="max" if on else "min",
             crossAlign="center", padding=3, w=40, h=24, sizingH="fixed", sizingV="fixed",
             cornerRadius=12, fills=[DARK if on else FAINT])


def select_box(value, w=None):
    return row("Select", T(value, 13, "Medium"), T("▾", 12, color=MUTE), gap=6,
               pad=[6, 10, 6, 10], sizingH="hug", fills=[SURF], strokes=[LINE],
               strokeWeight=1, cornerRadius=8)


# ---------------------------------------------------------------- phone chrome
def status_bar():
    return row("Status bar", T("9:41", 14, "Semi Bold"), spacer(), R(60, 12, FAINT, 6,
               name="Indicators"), pad=[14, 24, 6, 24])


def app_bar(title, sub=None, back=False, actions=()):
    texts = [T(title, 22 if not back else 17, "Bold" if not back else "Semi Bold")]
    if sub:
        texts.append(T(sub, 13, color=MUTE))
    kids = ([link(T("‹", 26, "Medium", name="Back"), "back")] if back else []) + [
        col("Title", *texts, gap=2)]
    kids += [icon(28, 8, name=f"Action / {a}") for a in actions]
    return row("App bar", *kids, pad=[8, 20, 12, 20])


def bottom_nav(active):
    items = []
    for name in ("Files", "Transfers", "Folders", "Settings"):
        on = name == active
        items.append(link(col(f"Tab / {name}", icon(24, 7, DARK if on else BLOCK),
                              T(name, 11, "Semi Bold" if on else "Medium", INK if on else MUTE),
                              gap=4, crossAlign="center", sizingH="fill"),
                          {"Files": "files-remote", "Transfers": "transfers",
                           "Folders": "folders", "Settings": "settings"}[name]))
    return row("Bottom nav", *items, gap=0, pad=[10, 8, 26, 8], fills=[SURF],
               strokes=[LINE], strokeWeight=1, strokeAlign="inside", align="min")


def phone(name, *body, nav=None, footer=None):
    content = col("Content", *body, gap=0, sizingV="fill", clip=True)
    kids = [status_bar(), content]
    if footer:
        kids.append(footer)
    if nav:
        kids.append(bottom_nav(nav))
    return F(name, *kids, w=PHONE[0], h=PHONE[1], layout="vertical", gap=0,
             fills=[BG], clip=True, cornerRadius=28)


def sheet(*kids):
    return col("Sheet", R(36, 4, FAINT, 2, name="Grabber"), *kids, gap=12,
               pad=[10, 20, 28, 20], fills=[SURF], cornerRadius=[20, 20, 0, 0],
               crossAlign="center")


def card(*kids, pad=0, gap=0):
    return col("Card", *kids, gap=gap, pad=pad, fills=[SURF], cornerRadius=14, clip=True)


def inset(*kids, pad=(0, 16, 0, 16), gap=12):
    return col("Group", *kids, gap=gap, pad=list(pad))


# ---------------------------------------------------------------- phone screens
def trust_row(net="Home", sub=None):
    return row("Trust network", checkbox(True),
               col("Text", T(f"Trust this network (Wi-Fi “{net}”)", 14, "Medium"),
                   T(sub or "LocalDrop only searches on networks you trust.", 12,
                     color=MUTE, wrap=True), gap=2),
               align="min", gap=10)


def p_join():
    return phone(
        "Phone / 01 Join group",
        col("Hero", icon(56, 16, DARK, "Logo"), T("Join or create your group", 26,
                                                  "Bold", wrap=True),
            T("Use the same group name and password on all your devices; the first one "
              "creates the group. Nobody else on the network can see them.", 15, color=MUTE,
              wrap=True),
            gap=10, pad=[24, 24, 20, 24]),
        col("Form",
            field("Group name", "home"),
            field("Password", "••••••••••••", hint="At least 10 characters, or use a "
                  "generated passphrase.", trailing="Generate"),
            field("This device's name", "Pixel 8"),
            trust_row(),
            gap=14, pad=[0, 24, 0, 24]),
        col("Fingerprint",
            row("Box", col("Text", T("Group fingerprint", 13, "Medium", MUTE),
                           T("3fa2-91c0", 22, "Bold", letterSpacing=1)),
                icon(28, 8, name="Copy"), pad=16, fills=[SURF], cornerRadius=12),
            T("Check that this code is the same on every device you add.", 12, color=MUTE,
              wrap=True),
            gap=8, pad=[18, 24, 0, 24]),
        footer=col("Footer", link(button("Join group", full=True), "perms"), gap=0,
                   pad=[16, 24, 34, 24]),
    )


def panel_tabs(active, peer="Laptop", online=True, peer_sub=None, disc=True):
    """Total Commander-style: left / right panel as two tabs; swipe to switch."""
    def tab(side, label, sub, on, dot=None):
        head = row("Label", *( [dot] if dot else []), T(label, 15, "Semi Bold" if on else
                                                         "Medium", INK if on else MUTE),
                   *( [link(T("▾", 14, color=MUTE, name="Pick device"), "devices")]
                      if side == "R" else []), gap=6, sizingH="hug")
        return link(col(f"Panel tab / {side}", head, T(sub, 11, color=MUTE),
                        R(10, 3, DARK if on else SURF, 1, name="Underline", sizingH="fill"),
                        gap=4, sizingH="fill", crossAlign="center", pad=[10, 0, 0, 0]),
                    "files-local" if side == "L" else
                    ("files-root" if online else "files-offline"))
    top = row("App row", T("LocalDrop", 15, "Bold"), T("home", 13, color=MUTE), spacer(),
              disc_pill(disc, phone=True), gap=8, pad=[2, 16, 2, 16])
    return col("Panel tabs", top, row("Tabs", tab("L", "This phone", "Left", active == 0),
                                 tab("R", peer, peer_sub or ("Right · online" if online
                                                             else "Right · offline"),
                                     active == 1, status_dot(online)), gap=0,
                                 pad=[0, 12, 0, 12]),
               R(10, 1, LINE, name="Baseline", sizingH="fill"), gap=0, fills=[SURF])


def path_bar(*parts, actions=("Search", "Sort"), up=None, search_to=None):
    back = T("‹", 22, "Medium", name="Up")
    return row("Path bar", link(back, up) if up else back, crumbs(*parts), spacer(),
               *[link(icon(24, 7, name=f"Action / {a}"), search_to)
                 if a == "Search" and search_to else icon(24, 7, name=f"Action / {a}")
                 for a in actions],
               gap=10, pad=[10, 16, 10, 16])


def p_row(name, sub, checked=None, lead=None, trailing=None, bar=None, hl=False):
    kids = []
    if checked is not None:
        kids.append(checkbox(checked))
    kids.append(lead or icon(36, 10))
    texts = [T(name, 15, "Medium"), T(sub, 12, color=MUTE)]
    if bar is not None:
        texts.append(progress(bar))
    kids.append(col("Text", *texts, gap=3))
    if trailing:
        kids.append(T(trailing, 18, "Medium", FAINT))
    return row(f"Row / {name}", *kids, pad=[10, 16, 10, 16], fills=[BG] if (checked or hl)
               else [])


def mini_transfer():
    return link(row("Transfer bar", icon(24, 7, DARK),
               col("Text", T("Sending Holiday 2024 · 34%", 13, "Semi Bold"), progress(0.34),
                   gap=5),
               chip("2 queued"), pad=[10, 16, 10, 16], fills=[SURF], strokes=[LINE],
               strokeWeight=1, strokeAlign="inside"), "transfers")


def cmd_bar(*items):
    """TC-style bottom command bar for the current selection."""
    cells = [col(f"Cmd / {n}", icon(22, 6, DARK if primary else BLOCK),
                 T(n, 11, "Semi Bold" if primary else "Medium", INK if primary else MUTE),
                 gap=4, crossAlign="center", sizingH="fill") for n, primary, *_ in items]
    cells = [link(c, it[2]) if len(it) > 2 else c for c, it in zip(cells, items)]
    return row("Command bar", *cells, gap=0, pad=[8, 8, 8, 8], fills=[SURF], strokes=[LINE],
               strokeWeight=1, strokeAlign="inside", align="min")


def p_files_remote():
    rows = card(
        p_row("2024", "Folder · 1 204 items", checked=False, trailing="›"),
        divider(100),
        p_row("2023", "Folder · 980 items", checked=False, trailing="›"),
        divider(100),
        p_row("IMG_0001.jpg", "4.1 MB · 12 Mar", checked=True),
        divider(100),
        p_row("IMG_0002.jpg", "3.8 MB · 12 Mar", checked=True),
        divider(100),
        p_row("VID_0003.mp4", "812 MB · 14 Mar", checked=False),
        divider(100),
        p_row("notes.txt", "2 KB · 15 Mar", checked=False),
    )
    return phone(
        "Phone / 02 Files — right tab (other device)",
        panel_tabs(1),
        path_bar("Photos", "Camera", up="files-root", search_to="search"),
        inset(row("Access", T("Your access:", 12, color=MUTE),
                  perm_chips("Browse", "Download", "Upload", "Delete → Trash"), gap=6),
              pad=(0, 16, 8, 16)),
        inset(rows, pad=(0, 12, 0, 12)),
        footer=col("Bottom", row("Selection", T("2 selected · 7.9 MB", 13, "Semi Bold"),
                                 spacer(), T("Select all", 13, "Medium", MUTE),
                                 pad=[8, 16, 8, 16], fills=[BG]),
                   cmd_bar(("Copy ← left", True), ("Move ← left", False), ("Delete", False, "delete"),
                           ("Rename", False), ("More", False)), gap=0),
        nav="Files",
    )


def p_files_offline():
    q = card(
        job("Photos for mom", "Folder · 48 files · 210 MB → Inbox", meta="Queued 2 h ago",
            action="⋮"),
        divider(14),
        job("Report Q3.pdf", "2.1 MB → Projects", meta="Queued 40 min ago", action="⋮"),
        divider(14),
        job("VID_0412.mp4", "1.8 GB → Inbox", meta="Queued 5 min ago · copied to app cache",
            action="⋮"),
    )
    note = row("Offline note", icon(32, 9),
               col("Text", T("iPad is offline", 15, "Semi Bold"),
                   T("Last seen 2 h ago. Its folders aren't available. Files below are sent "
                     "automatically when you're both on the same network.", 13, color=MUTE,
                     wrap=True), gap=3),
               pad=14, align="min", fills=[SURF], cornerRadius=14)
    hint = row("Background hint", icon(20, 6), T("Keep LocalDrop open (or allow background "
               "activity) so the queue can start when iPad appears.", 12, color=MUTE,
               wrap=True), pad=12, cornerRadius=12, strokes=[LINE], strokeWeight=1)
    return phone(
        "Phone / 08 Files — right tab, device offline (queue)",
        panel_tabs(1, "iPad", online=False, peer_sub="Right · offline · 3 queued"),
        inset(note, pad=(12, 12, 0, 12)),
        section("Waiting to send · 3", "Edit order"),
        inset(q, pad=(0, 12, 0, 12)),
        inset(hint, pad=(12, 12, 0, 12)),
        footer=col("Footer", row("Buttons", button("Clear", primary=False),
                                 link(button("+  Add files", full=True), "send"), gap=10),
                   gap=0, pad=[12, 16, 12, 16], fills=[SURF], strokes=[LINE], strokeWeight=1,
                   strokeAlign="inside"),
        nav="Files",
    )


def glyph_icon(ch, size=36, radius=10):
    return F(f"Icon {ch}", T(ch, 14, "Bold", SURF), layout="horizontal", mainAlign="center",
             crossAlign="center", w=size, h=size, sizingH="fixed", sizingV="fixed",
             cornerRadius=radius, fills=[DARK])


def p_files_root():
    """Right tab at the other device's root: only what it shares with us, plus its Inbox."""
    shares = card(
        link(p_row("Photos", "Browse · Download · Upload · Delete → Trash", trailing="›"),
             "files-remote"),
        divider(68),
        p_row("Projects", "Browse · Download", trailing="›"),
        divider(68),
        p_row("Music", "Browse · Download", trailing="›"),
    )
    return phone(
        "Phone / 09 Files — right tab, device root",
        panel_tabs(1),
        path_bar("Dmytro's Laptop", actions=("Search",), search_to="search"),
        inset(row("Device", F("Avatar", R(36, 36, BLOCK, 18), status_dot(True),
                              layout="horizontal", gap=-10, crossAlign="max", sizingH="hug",
                              sizingV="hug"),
                  col("Text", T("Dmytro's Laptop", 15, "Semi Bold"),
                      T("Windows · Online · 3 shared folders", 12, color=MUTE),
                      gap=2),
                  link(T("Switch", 13, "Semi Bold", INK, name="Switch device"), "devices"),
                  pad=14, fills=[SURF], cornerRadius=14), pad=(4, 12, 0, 12)),
        section("Send to this device"),
        inset(card(link(p_row("Inbox", "Upload only · lands in Downloads/LocalDrop",
                              lead=glyph_icon("↓"), trailing="›"), "send")),
              pad=(0, 12, 0, 12)),
        section("Shared with you", "3"),
        inset(shares, pad=(0, 12, 0, 12)),
        inset(T("Only folders this device shares with you are listed.", 12, color=MUTE,
                wrap=True), pad=(10, 20, 0, 20)),
        footer=mini_transfer(),
        nav="Files",
    )


def p_devices_sheet():
    """Device picker for the right tab (desktop: the sidebar device list)."""
    dev = lambda n, s, on, to, sel=False, t=None: link(list_row(  # noqa: E731
        n, s, lead=F("Avatar", R(36, 36, BLOCK, 18), status_dot(on), layout="horizontal",
                     gap=-10, crossAlign="max", sizingH="hug", sizingV="hug"),
        trailing=t or (radio(True) if sel else None), pad_x=4), to)
    body = sheet(
        row("Head", T("Choose device", 20, "Bold"), spacer(),
            link(T("Close", 14, "Semi Bold", MUTE), "back")),
        col("Lists",
            T("ONLINE", 11, "Semi Bold", MUTE, letterSpacing=0.6),
            dev("Dmytro's Laptop", "Windows · Online", True, "files-root", sel=True),
            dev("Desk PC", "Linux · Online · receiving", True, "files-root-empty"),
            T("KNOWN · OFFLINE", 11, "Semi Bold", MUTE, letterSpacing=0.6),
            dev("iPad", "Last seen 2 h ago", False, "files-offline", t=chip("3 queued")),
            dev("Work MacBook", "Last seen yesterday", False, "files-offline"),
            gap=6),
        row("Add hint", icon(20, 6), T("To add a device, join the same group on it (name, "
            "password, fingerprint 3fa2-91c0).", 12, color=MUTE, wrap=True), pad=12,
            cornerRadius=12, strokes=[LINE], strokeWeight=1),
        link(T("Connect by address…", 14, "Semi Bold", name="Connect by address"), "connect"),
    )
    dim = F("Scrim", w=PHONE[0], h=10, sizingH="fill", sizingV="fill", fills=["#18181B66"])
    return F("Phone / 10 Device picker", status_bar(), dim, body, w=PHONE[0], h=PHONE[1],
             layout="vertical", gap=0, fills=[BLOCK], clip=True, cornerRadius=28)


def p_folders():
    """Folders tab: this phone's whitelist (desktop: Settings › Shared folders list)."""
    rows = card(
        link(list_row("Camera", "DCIM/Camera · 2 devices · read & write", trailing="›",
                      pad_x=16), "share"),
        divider(68),
        list_row("Documents", "All members · read only", trailing="›", pad_x=16),
        divider(68),
        list_row("Music", "Desk PC · upload only", trailing="›", pad_x=16),
    )
    return phone(
        "Phone / 11 Shared folders",
        app_bar("Shared folders", "What your group can see on this phone", actions=("Add",)),
        inset(rows),
        inset(row("Hint", icon(20, 6), T("Only these folders are visible to other devices. "
                  "Each one follows App defaults unless you change it.", 13, color=MUTE,
                  wrap=True), pad=14, cornerRadius=12, strokes=[LINE], strokeWeight=1),
              pad=(16, 16, 0, 16)),
        section("Receiving"),
        inset(card(list_row("Inbox", "Download/LocalDrop · files sent to you",
                            trailing="›", pad_x=16))),
        footer=row("FAB row", spacer(), link(button("+  Add folder"), "add-share"),
                   pad=[0, 20, 16, 20]),
        nav="Folders",
    )


def default_row(title, value):
    return list_row(title, None, lead=None, trailing=select_box(value), pad_x=16)


def p_settings():
    """Settings (desktop: Settings window tabs General / App defaults / Devices)."""
    general = card(
        link(list_row("Group", "home · fingerprint 3fa2-91c0", lead=None, trailing="›",
                      pad_x=16), "group"),
        divider(16),
        link(list_row("Networks", "Trusted: Home · discovering", lead=None,
                      trailing="›", pad_x=16), "networks"),
        divider(16),
        list_row("Visible to your group", None, lead=None, trailing=toggle(True), pad_x=16),
        divider(16),
        list_row("Run in background", "Needed to send queued files", lead=None,
                 trailing=toggle(True), pad_x=16),
    )
    defaults = card(
        default_row("Accept files", "Automatically"), divider(16),
        default_row("Delete by others", "Trash"), divider(16),
        default_row("Conflicts", "Ask"),
    )
    more = card(
        link(list_row("Shared folders", "3 folders", lead=None, trailing="›", pad_x=16),
             "folders"),
        divider(16),
        link(list_row("Known devices", "4 devices · remove or rename", lead=None,
                      trailing="›", pad_x=16), "known"),
        divider(16),
        link(list_row("Backups", "3 sets · 412 MB", lead=None, trailing="›", pad_x=16),
             "backups"),
    )
    return phone(
        "Phone / 12 Settings",
        app_bar("Settings"),
        section("General"), inset(general),
        section("App defaults", "Used by every shared folder"), inset(defaults),
        section("More"), inset(more),
        nav="Settings",
    )


def p_files_local():
    star = F("Shared icon", T("★", 14, "Bold", SURF), layout="horizontal", mainAlign="center",
             crossAlign="center", w=36, h=36, sizingH="fixed", sizingV="fixed",
             cornerRadius=10, fills=[DARK])
    return phone(
        "Phone / 05 Files — left tab (this phone)",
        panel_tabs(0),
        path_bar("This phone", actions=("Search",)),
        section("Shared with your group"),
        inset(card(link(p_row("Shared folders", "3 folders · Camera, Documents, Music",
                              lead=star, trailing="›"), "share"))),
        section("Storage"),
        inset(card(p_row("Internal storage", "41 GB free of 128 GB", bar=0.68, trailing="›"),
                   divider(68),
                   p_row("SD card", "88 GB free of 128 GB", bar=0.31, trailing="›"))),
        section("Places"),
        inset(card(p_row("Downloads", "84 items", trailing="›"), divider(68),
                   link(p_row("DCIM / Camera", "2 410 items", trailing="›"), "local-folder"), divider(68),
                   p_row("Documents", "37 items", trailing="›"))),
        footer=mini_transfer(),
        nav="Files",
    )


def p_send():
    files = card(
        list_row("Holiday 2024", "Folder · 1 820 files · 3.1 GB", trailing="×", pad_x=16),
        divider(68),
        list_row("contract.pdf", "2.4 MB", trailing="×", pad_x=16),
        divider(68),
        list_row("IMG_0142.MOV", "1.2 GB", trailing="×", pad_x=16),
    )
    targets = card(
        list_row("Dmytro's Laptop", "Online", lead=radio(True), pad_x=16),
        divider(52),
        list_row("Desk PC", "Online", lead=radio(False), pad_x=16),
        divider(52),
        list_row("iPad", "Offline · will be sent when it's back", lead=radio(False),
                 pad_x=16),
    )
    return phone(
        "Phone / 03 Send (from Share)",
        app_bar("Send 3 items", "4.3 GB in total", back=True),
        section("Items", "+ Add"), inset(files),
        section("To"), inset(targets),
        section("Destination"),
        inset(row("Destination", col("Text", T("Inbox", 15, "Medium"),
                                    T("Downloads/LocalDrop on Laptop", 13, color=MUTE)),
                  select_box("Change"), pad=14, fills=[SURF], cornerRadius=14)),
        footer=col("Footer", link(button("Send now", full=True), "transfers"), gap=0, pad=[16, 20, 34, 20],
                   fills=[SURF]),
    )


def job(title, sub, frac=None, state=None, action=None, meta=None):
    head = row("Head", icon(32, 8), col("Text", T(title, 15, "Medium"), T(sub, 12, color=MUTE),
                                       gap=2),
               action if isinstance(action, dict) else T(action or "×", 18, "Medium", MUTE),
               align="center")
    kids = [head]
    if frac is not None:
        kids.append(progress(frac))
    if meta or state:
        kids.append(row("Meta", T(meta or "", 12, color=MUTE), spacer(),
                        *( [chip(state)] if state else [])))
    return col(f"Job / {title}", *kids, gap=10, pad=14)


def p_transfers():
    laptop = card(
        link(job("Holiday 2024", "Folder · 412 of 1 820 files", 0.34, meta="1.1 of 3.1 GB · "
                 "11.4 MB/s · 3 min left", action="×"), "details"),
        divider(14),
        job("contract.pdf", "2.4 MB", meta="Next in queue", action="×"),
    )
    ipad = card(link(job("Photos for mom", "48 files · 210 MB", state="Waiting for device",
                    meta="Queued 2 h ago", action="×"), "files-offline"))
    incoming = card(
        job("Backup.zip from Desk PC", "18.0 GB · into Folders/Backups", 0.72,
            meta="13.0 of 18.0 GB · 11.6 MB/s",
            action=link(T("⋮", 18, "Medium", MUTE, name="More"), "job-menu")),
        divider(14),
        job("Docs from Work MacBook", "12 files already exist", state="Needs decision",
            action=link(button("Resolve", small=True), "conflict")),
    )
    return phone(
        "Phone / 04 Transfers",
        app_bar("Transfers", "2 active · 3 waiting", actions=("Filter",)),
        inset(row("Tabs", chip("All", True), chip("Outgoing"), chip("Incoming"), spacer(),
                  link(T("History  ›", 13, "Semi Bold", name="History"), "history"), gap=8), pad=(4, 20, 4, 20)),
        section("To Dmytro's Laptop", "Sending"), inset(laptop),
        section("To iPad", "Offline"), inset(ipad),
        section("Incoming"), inset(incoming),
        nav="Transfers",
    )


def option(title, sub, selected=False, disabled=False):
    c = FAINT if disabled else INK
    r = row(f"Option / {title}", radio(selected),
            col("Text", T(title, 15, "Medium", c), T(sub, 12, color=MUTE, wrap=True), gap=2),
            pad=14, align="min", cornerRadius=12, fills=[BG if selected else SURF],
            strokes=[DARK if selected else LINE], strokeWeight=1.5 if selected else 1)
    return r


def p_conflict():
    body = sheet(
        col("Head", T("12 files already exist", 20, "Bold"),
            T("Uploading “Docs” to Work MacBook › Projects. 230 new files will be added "
              "either way.", 14, color=MUTE, wrap=True), gap=6),
        col("Options",
            option("Upload only new files", "Keep the 12 existing files as they are.",
                   selected=True),
            option("Replace, keep a backup", "Old versions go to the folder's backups."),
            option("Replace conflicting files", "Old versions are moved to the Trash."),
            link(option("Replace the whole folder", "Also removes files that aren't in this "
                        "upload (Trash)."), "mirror"),
            gap=8),
        T("Show the 12 files", 13, "Semi Bold", INK),
        row("Actions", link(button("Cancel", primary=False, full=True), "back"),
            link(button("Continue", full=True), "back"), gap=10),
    )
    dim = F("Scrim", w=PHONE[0], h=10, sizingH="fill", sizingV="fill", fills=["#18181B66"])
    return F("Phone / 06 Conflict dialog", status_bar(), dim, body, w=PHONE[0], h=PHONE[1],
             layout="vertical", gap=0, fills=[BLOCK], clip=True, cornerRadius=28)


def setting_row(title, value, inherited=True):
    sub = "From app settings" if inherited else "Set for this folder"
    return list_row(title, sub, lead=None, trailing=select_box(value), pad_x=16)


def p_share_settings():
    access = card(
        list_row("All group members", None, lead=radio(False), pad_x=16),
        divider(52),
        list_row("Selected devices", "Dmytro's Laptop, Desk PC", lead=radio(True), pad_x=16),
    )
    perms = card(
        setting_row("Browse", "Inherit · Allow"),
        divider(16),
        setting_row("Download", "Inherit · Allow"),
        divider(16),
        setting_row("Upload", "Allow", inherited=False),
        divider(16),
        setting_row("Delete", "Inherit · Trash"),
        divider(16),
        setting_row("Conflicts", "Inherit · Ask"),
    )
    return phone(
        "Phone / 07 Shared folder settings",
        app_bar("Photos", "Internal storage › DCIM", back=True),
        section("Who can access"), inset(access),
        section("Permissions", "Reset to app defaults"), inset(perms),
        section("Backups"),
        inset(card(link(list_row("Backups of replaced files", "3 sets · 412 MB", lead="icon",
                                 trailing="›", pad_x=16), "backups"))),
        inset(button("Remove from shared folders", primary=False, full=True),
              pad=(20, 16, 0, 16)),
        nav="Folders",
    )


# ---------------------------------------------------------------- desktop
def side_item(label, active=False, lead="icon", trailing=None, sub=None, to=None):
    kids = []
    if lead == "icon":
        kids.append(icon(20, 6, DARK if active else BLOCK))
    elif lead:
        kids.append(lead)
    texts = [T(label, 14, "Semi Bold" if active else "Medium", INK)]
    if sub:
        texts.append(T(sub, 12, color=MUTE))
    kids.append(col("Text", *texts, gap=1))
    if trailing:
        kids.append(trailing if isinstance(trailing, dict) else T(trailing, 12, "Medium", MUTE))
    r = row(f"Nav / {label}", *kids, gap=10, pad=[8, 10, 8, 10], cornerRadius=8,
            fills=[BLOCK] if active else [])
    return link(r, to) if to else r


def sidebar(active, disc=True):
    idle = active == "transfers-empty"   # nothing queued anywhere
    if idle:
        active = "transfers"
    dev = lambda n, s, on, a=False, t=None, to=None: side_item(  # noqa: E731
        n, a, lead=F("Avatar", R(28, 28, BLOCK, 14), status_dot(on), layout="horizontal",
                     gap=-8, crossAlign="max", sizingH="hug", sizingV="hug"),
        sub=s, trailing=t, to=to)
    return col(
        "Sidebar",
        row("Brand", icon(28, 8, DARK), col("Text", T("LocalDrop", 15, "Bold"),
                                            T("home · 3fa2-91c0", 12, color=MUTE), gap=0),
            pad=[4, 6, 12, 6]),
        *(([T("DEVICES", 11, "Semi Bold", MUTE, letterSpacing=0.6),
            side_item("Searching on Home…", True, lead=icon(28, 14, BLOCK),
                      sub="No devices yet")]) if disc == "empty" else
          ([T("DEVICES", 11, "Semi Bold", MUTE, letterSpacing=0.6),
            dev("Dmytro's Laptop", "Online", True, active == "laptop", to="d-remote"),
            dev("Desk PC", "Online" if idle else "Online · receiving", True, active == "desk",
                to="d-remote-empty"),
            T("KNOWN · OFFLINE", 11, "Semi Bold", MUTE, letterSpacing=0.6)] if disc else
           [T("KNOWN · NOT SEARCHING", 11, "Semi Bold", MUTE, letterSpacing=0.6),
            dev("Dmytro's Laptop", "Status unknown", False, active == "laptop"),
            dev("Desk PC", "Status unknown", False)]) + [
            dev("iPad", "2 h ago" if idle else "2 h ago · 3 queued", False, active == "ipad",
                t=None if idle else chip("3"), to="d-offline"),
            dev("Work MacBook", "Yesterday", False)]),
        F("Fill", layout="vertical", sizingH="fill", sizingV="fill"),
        side_item("Transfers", active == "transfers",
                  trailing=None if disc == "empty" or idle else chip("2", True),
                  to="d-transfers"),
        side_item("Settings", active == "settings", to="d-general"),
        gap=4, pad=[16, 12, 16, 12], sizingH="fixed", sizingV="fill", w=264,
        fills=[SURF], strokes=[LINE], strokeWeight=1, strokeAlign="inside",
    )


def small_toggle(on):
    knob = R(12, 12, SURF, 6, name="Knob")
    return F("Toggle", knob, layout="horizontal", mainAlign="max" if on else "min",
             crossAlign="center", padding=2, w=28, h=16, sizingH="fixed", sizingV="fixed",
             cornerRadius=8, fills=[DARK if on else FAINT])


def disc_pill(on=True, phone=False, net="Home"):
    """Main-screen switch for LAN discovery + the network it runs on.

    Label part opens the network menu, the toggle switches discovery on/off.
    """
    label = row("Network", R(8, 8, DARK if on else FAINT, 4, name="Dot"),
                T(f"Discovery · {net}" if on else "Discovery off", 13, "Semi Bold",
                  INK if on else MUTE),
                T("▾", 11, color=MUTE), gap=6, sizingH="hug")
    sep = R(1, 16, LINE, name="Sep")
    if phone:
        label, tog = link(label, "networks"), link(small_toggle(on),
                                                   "disc-off" if on else "files-root")
    else:
        label, tog = link(label, "d-net-menu"), link(small_toggle(on),
                                                     "d-disc-off" if on else "d-remote")
    return row("Discovery", label, sep, tog, gap=8, pad=[6, 8, 6, 10], sizingH="hug",
               fills=[SURF], strokes=[LINE], strokeWeight=1, cornerRadius=999)


def off_state(title, body, button_to, phone=False):
    return col("Discovery off", icon(48 if not phone else 40, 14, BLOCK),
               T(title, 18 if not phone else 17, "Bold"),
               T(body, 13, color=MUTE, wrap=True, textAlignHorizontal="center"),
               link(button("Turn on discovery", full=phone), button_to),
               gap=12, pad=28 if not phone else 20, crossAlign="center", fills=[SURF],
               cornerRadius=16)


def side_toggle(on):
    """Shows / hides the device sidebar (two view states)."""
    glyph = F("Glyph", R(5, 14, DARK if on else FAINT, 1, name="Side"),
              R(9, 14, SURF, 1, name="Main"), layout="horizontal", gap=1, sizingH="hug",
              sizingV="hug", padding=1, cornerRadius=3, strokes=[DARK if on else FAINT],
              strokeWeight=1.5)
    return F("Sidebar toggle", glyph, layout="horizontal", mainAlign="center",
             crossAlign="center", w=32, h=32, sizingH="fixed", sizingV="fixed", cornerRadius=8,
             fills=[BLOCK] if on else [SURF], strokes=[LINE], strokeWeight=1)


def window(name, side, main, disc=True):
    title = row("Title bar", R(12, 12, FAINT, 6, name="Close"), R(12, 12, FAINT, 6, name="Min"),
                R(12, 12, FAINT, 6, name="Max"), spacer(), T("LocalDrop", 12, "Medium", MUTE),
                spacer(), R(52, 12, BG, name="Balance"), gap=8, pad=[10, 14, 10, 14],
                fills=[BLOCK])
    body = row("Body", *( [sidebar(side, disc)] if side else []), main, gap=0, align="min",
               sizingV="fill")
    return F(name, title, body, w=DESK[0], h=DESK[1], layout="vertical", gap=0, fills=[BG],
             clip=True, cornerRadius=12, strokes=[LINE], strokeWeight=1)


def main_area(*kids, gap=20):
    return col("Main", *kids, gap=gap, pad=[24, 32, 24, 32], sizingV="fill", clip=True)


SETTINGS_TABS = ("General", "Shared folders", "App defaults", "Devices", "Networks",
                 "Backups")
SETTINGS_LINKS = {"General": "d-general", "Shared folders": "d-settings",
                  "App defaults": "d-defaults", "Devices": "d-devices", "Networks": "d-networks",
                  "Backups": "d-backups"}


def tabs(*names, active=0, links=None):
    items = []
    for i, n in enumerate(names):
        on = i == active
        t = col(f"Tab / {n}", T(n, 14, "Semi Bold" if on else "Medium", INK if on else MUTE),
                R(10, 2, DARK if on else BG, 1, name="Underline", sizingH="fill"),
                gap=8, sizingH="hug")
        items.append(link(t, links[n]) if links and n in links and not on else t)
    bar = F("Tab items", *items, layout="horizontal", gap=24, sizingH="fill", sizingV="hug")
    return col("Tabs", bar, R(10, 1, LINE, name="Baseline", sizingH="fill"), gap=0)


def seg(*opts, active=0, links=None):
    links = {"Remote": "d-remote", "Split": "d-split"} if links is None else links
    items = []
    for i, o in enumerate(opts):
        on = i == active
        items.append(link(row(f"Segment / {o}", icon(14, 3, SURF if on else FAINT),
                         T(o, 13, "Semi Bold" if on else "Medium", SURF if on else INK),
                         gap=6, pad=[6, 12, 6, 12], sizingH="hug", fills=[DARK] if on else [],
                         cornerRadius=7), links.get(o)))
    return row("View mode", *items, gap=2, pad=3, sizingH="hug", fills=[BLOCK], cornerRadius=10)


def crumbs(*parts):
    kids = []
    for i, p in enumerate(parts):
        if i:
            kids.append(T("›", 13, color=FAINT))
        last = i == len(parts) - 1
        kids.append(T(p, 13 if not last else 14, "Semi Bold" if last else "Medium",
                      INK if last else MUTE))
    return row("Breadcrumb", *kids, gap=6, sizingH="hug")


def nav_btns():
    b = lambda s: F(f"Nav / {s}", T(s, 15, "Medium", MUTE), layout="horizontal",  # noqa: E731
                    mainAlign="center", crossAlign="center", w=28, h=28, sizingH="fixed",
                    sizingV="fixed", cornerRadius=7, strokes=[LINE], strokeWeight=1)
    return row("History", b("‹"), b("›"), b("↑"), gap=4, sizingH="hug")


def search(w=170):
    return row("Search", icon(14, 4, FAINT), T("Search", 13, color=FAINT), gap=8,
               pad=[7, 10, 7, 10], sizingH="fixed", w=w, fills=[SURF], strokes=[LINE],
               strokeWeight=1, cornerRadius=8)


def no_box():
    return F("No box", layout="horizontal", w=20, h=20, sizingH="fixed", sizingV="fixed")


def frow(name, cells=(), widths=(), checked=None, lead=None, sub=None, header=False, bar=None,
         trailing=None, hl=False, pad_x=14, box=True):
    """One file-list row: [checkbox] icon name/sub [cells...] [trailing]."""
    kids = [checkbox(checked) if checked is not None else no_box()] if box else []
    texts = [T(name, 12 if header else 14, "Semi Bold" if header else "Medium",
               MUTE if header else INK)]
    if sub:
        texts.append(T(sub, 12, color=MUTE))
    if bar is not None:
        texts.append(progress(bar))
    name_kids = ([] if header else [lead or icon(24, 6)]) + [col("Text", *texts, gap=4)]
    kids.append(row("Name", *name_kids, gap=10))
    for i, (c, w) in enumerate(zip(cells, widths)):
        cell = c if isinstance(c, dict) else T(c, 12 if header else 13,
                                                "Semi Bold" if header else "Regular", MUTE)
        kids.append(F(f"Cell {i}", cell, layout="horizontal", sizingH="fixed", sizingV="hug",
                      w=w, gap=4))
    if trailing:
        kids.append(T(trailing, 14, "Medium", FAINT))
    return row(f"Row / {name}", *kids, gap=14, pad=[9, pad_x, 9, pad_x],
               fills=[BG] if (checked or hl) else [])


def pane_section(title, trailing=None):
    kids = [T(title.upper(), 11, "Semi Bold", MUTE, letterSpacing=0.6), spacer()]
    if trailing:
        kids.append(T(trailing, 12, "Medium", MUTE))
    return row(f"Section / {title}", *kids, pad=[14, 14, 4, 14])


def fill_v(name="Fill"):
    return F(name, layout="vertical", sizingH="fill", sizingV="fill")


def transfer_bar():
    return row(
        "Transfer bar", icon(28, 8, DARK),
        col("Text", row("Line", T("Sending Holiday 2024 → Dmytro's Laptop", 13, "Semi Bold"),
                        spacer(), T("34% · 11.4 MB/s · 3 min left", 12, color=MUTE)),
            progress(0.34), gap=6),
        chip("2 queued"), link(chip("1 needs decision", True), "d-conflict"),
        link(T("Transfers  ›", 13, "Semi Bold", INK), "d-transfers"),
        gap=14, pad=[10, 16, 10, 16], fills=[SURF], cornerRadius=12, strokes=[LINE],
        strokeWeight=1)


def perm_chips(*names):
    return row("Access", *[T(n, 12, "Medium", MUTE) for n in names], gap=8, sizingH="hug")


def d_remote(name="Desktop / 01 Browse — remote only"):
    """Main window, mode 1: only the other PC's folders (its whitelist + Inbox)."""
    w = (90, 270, 100)
    inbox_lead = F("Inbox icon", T("↓", 14, "Bold", SURF), layout="horizontal",
                   mainAlign="center", crossAlign="center", w=24, h=24, sizingH="fixed",
                   sizingV="fixed", cornerRadius=6, fills=[DARK])
    tbl = card(
        frow("Name", ("Size", "Your access", "Modified"), w, header=True),
        divider(0),
        frow("Inbox", ("—", perm_chips("Upload only"), "—"), w, checked=False,
             lead=inbox_lead, sub="Files you send land in Downloads/LocalDrop", trailing="›"),
        divider(48),
        link(frow("Photos", ("2 184 items", perm_chips("Browse", "Download", "Upload",
             "Delete → Trash"), "Today, 09:12"), w, checked=False, sub="Pictures",
             trailing="›"), "d-remote-folder"),
        divider(48),
        frow("Projects", ("412 items", perm_chips("Browse", "Download"), "Yesterday"), w,
             checked=True, sub="D:\\Work\\Projects", trailing="›"),
        divider(48),
        frow("Music", ("3 020 items", perm_chips("Browse", "Download"), "2 Mar"), w,
             checked=False, sub="Music", trailing="›"),
    )
    head = row("Device head", F("Avatar", R(40, 40, BLOCK, 20), status_dot(True),
                                layout="horizontal", gap=-10, crossAlign="max", sizingH="hug",
                                sizingV="hug"),
               col("Text", T("Dmytro's Laptop", 22, "Bold"),
                   T("Windows · Online · shares 3 folders with you", 13, color=MUTE), gap=2),
               link(button("Send files…", primary=False, small=True), "d-send"),
               gap=14)
    toolbar = row("Toolbar", side_toggle(True), nav_btns(), crumbs("Dmytro's Laptop"), spacer(),
                  disc_pill(), link(search(), "d-search"),
                  seg("Remote", "Split", active=0), gap=12)
    actions = row("Actions", T("Only folders this device shares with you are listed.", 13,
                               color=MUTE), spacer(),
                  T("1 selected · 412 items", 13, "Medium", MUTE),
                  button("Open", primary=False, small=True), button("Download", small=True),
                  gap=8)
    hint = link(row("Drop hint", icon(20, 6), T("Drop files on Inbox or on a folder you can "
                    "upload to. If the device goes offline, they wait in the queue.", 13,
                    color=MUTE, wrap=True), pad=14, cornerRadius=12, strokes=[LINE],
                    strokeWeight=1), "d-drop")
    return window(name, "laptop", main_area(
        toolbar, head, actions, tbl, hint, fill_v(), transfer_bar(), gap=16,
    ))


def d_split(side=False):
    """Main window, mode 2: this PC on the left, the other PC on the right."""
    w = (80, 70) if side else (90, 110, 120)
    shared_lead = F("Shared icon", T("★", 12, "Bold", SURF), layout="horizontal",
                    mainAlign="center", crossAlign="center", w=24, h=24, sizingH="fixed",
                    sizingV="fixed", cornerRadius=6, fills=[DARK])

    def pane(title, crumb, body, footer, active=False):
        head = row("Pane head", nav_btns(), crumb, pad=[10, 12, 10, 12], gap=10)
        return col(f"Pane / {title}", head, R(10, 1, LINE, name="Line", sizingH="fill"), *body,
                   fill_v(), R(10, 1, LINE, name="Line", sizingH="fill"),
                   row("Pane footer", T(footer, 12, "Medium", MUTE), pad=[8, 14, 8, 14]),
                   gap=0, sizingV="fill", fills=[SURF], cornerRadius=12, clip=True,
                   strokes=[DARK if active else LINE], strokeWeight=1.5 if active else 1)

    left = pane("THIS PC", crumbs("This PC"), [
        pane_section("Shared with your group"),
        frow("Shared folders", lead=shared_lead, sub="3 folders · Photos, Projects, Backups",
             trailing="›", hl=True, box=False),
        pane_section("Drives"),
        frow("Local Disk (C:)", sub="212 GB free of 512 GB", bar=0.59, trailing="›", box=False),
        divider(48 - 34),
        frow("Data (D:)", sub="1.4 TB free of 2 TB", bar=0.3, trailing="›", box=False),
        divider(48 - 34),
        frow("USB Drive (E:)", sub="12 GB free of 64 GB", bar=0.81, trailing="›", box=False),
        pane_section("Places"),
        frow("Desktop", trailing="›", box=False), frow("Downloads", trailing="›", box=False),
        frow("Documents", trailing="›", box=False),
    ], "3 drives · 3 places")

    device_pick = row("Device", status_dot(True), T("Dmytro's Laptop", 13, "Semi Bold"),
                      T("▾", 12, color=MUTE), gap=6, pad=[5, 10, 5, 10], sizingH="hug",
                      fills=[BG], cornerRadius=8, strokes=[LINE], strokeWeight=1)
    right = pane("DMYTRO'S LAPTOP", row("Path", device_pick, crumbs("Photos", "Camera"),
                                        gap=8, sizingH="hug"), [
        row("Access", T("Your access:", 12, color=MUTE), perm_chips("Browse", "Download",
            "Upload", "Delete → Trash"), pad=[8, 14, 4, 14], gap=8),
        *[x for i, (n, c, chk) in enumerate([
            ("Name", ("Size", "Type", "Modified"), None),
            ("2024", ("1 204 items", "Folder", "Today"), False),
            ("2023", ("980 items", "Folder", "3 Jan"), False),
            ("IMG_0001.jpg", ("4.1 MB", "JPEG image", "12 Mar"), True),
            ("IMG_0002.jpg", ("3.8 MB", "JPEG image", "12 Mar"), True),
            ("VID_0003.mp4", ("812 MB", "MP4 video", "14 Mar"), False),
            ("notes.txt", ("2 KB", "Text", "15 Mar"), False),
        ]) for x in ([divider(0 if i == 1 else 48)] if i else []) + [
            frow(n, (c[0], c[2]) if side else c, w, header=chk is None,
                 checked=chk, trailing="›" if c[1] == "Folder" else None)]],
    ], "6 items · 2 selected · 7.9 MB", active=True)

    toolbar = row("Toolbar", link(side_toggle(side), "d-split" if side else "d-split-side"),
                  *([] if side else [T("This PC", 15, "Semi Bold"), T("⇄", 15, "Medium", MUTE),
                                     T("Dmytro's Laptop", 15, "Semi Bold")]), spacer(),
                  disc_pill(), search(120 if side else 170),
                  seg("Remote", "Split", active=1), gap=10)
    actions = row("Copy bar", button("‹  Download to This PC", small=True),
                  *([] if side else [T("Drag between panes to copy · Delete follows the "
                                       "folder's setting", 12, color=MUTE)]),
                  button("Send to Dmytro's Laptop  ›", primary=False, small=True),
                  link(button("Delete", primary=False, small=True), "d-delete"),
                  gap=12, mainAlign="center")
    return window("Desktop / 31 Browse — split view, sidebar shown" if side else
                  "Desktop / 02 Browse — split view, sidebar hidden",
                  "laptop" if side else None, main_area(
        toolbar,
        row("Panes", left, right, gap=12, align="min", sizingV="fill"),
        actions, transfer_bar(), gap=14,
    ))


def d_offline():
    """Selected device is offline: show the send-ahead queue to it instead of its folders."""
    w = (110, 150, 40)
    jobs = card(
        row("Card head", T("Waiting to send · 3", 15, "Semi Bold"), spacer(),
            T("Drag to reorder", 12, color=MUTE), pad=[14, 16, 8, 16]),
        frow("Name", ("Size", "Destination", ""), w, header=True),
        divider(0),
        frow("Photos for mom", ("210 MB", "Inbox", "×"), w, sub="Folder · 48 files · queued "
             "2 h ago"),
        divider(48),
        frow("Report Q3.pdf", ("2.1 MB", "Projects", "×"), w, sub="Queued 40 min ago"),
        divider(48),
        frow("Movies", ("4.2 GB", "Inbox", "×"), w, sub="Folder · 6 files · queued 5 min ago "
             "· path reference"),
    )
    head = row("Device head", F("Avatar", R(40, 40, BLOCK, 20), status_dot(False),
                                layout="horizontal", gap=-10, crossAlign="max", sizingH="hug",
                                sizingV="hug"),
               col("Text", T("iPad", 22, "Bold"),
                   T("iPadOS · Offline · last seen 2 h ago", 13, color=MUTE), gap=2),
               link(button("Add files…", primary=False, small=True), "d-send"),
               button("Clear queue", primary=False, small=True), gap=14)
    note = row("Offline note", icon(20, 6), col("Text",
               T("iPad is offline, so its folders aren't available.", 14, "Semi Bold"),
               T("Files you drop here are queued and sent automatically when iPad and this PC "
                 "are on the same network. Destination: Inbox, or a shared folder you could "
                 "upload to last time.", 13, color=MUTE, wrap=True), gap=4),
               pad=16, align="min", cornerRadius=12, fills=[SURF], strokes=[LINE],
               strokeWeight=1)
    drop = col("Drop zone", icon(36, 10), T("Drop files or folders to queue them for iPad", 15,
                                             "Semi Bold"),
               row("Destination", T("Destination:", 13, color=MUTE), select_box("Inbox"),
                   gap=8, sizingH="hug"),
               gap=10, pad=24, crossAlign="center", cornerRadius=14, strokes=[FAINT],
               strokeWeight=1.5)
    toolbar = row("Toolbar", side_toggle(True), nav_btns(), crumbs("iPad"), spacer(), disc_pill(),
                  search(),
                  seg("Remote", "Split", active=0), gap=12)
    return window("Desktop / 03 Device offline — send queue", "ipad", main_area(
        toolbar, head, note, jobs, drop, fill_v(), transfer_bar(), gap=16,
    ))


def d_settings():
    shares = col(
        "Share list",
        row("Head", T("Shared folders", 15, "Semi Bold"), spacer(),
            link(button("+ Add folder", primary=False, small=True), "d-add-share")),
        side_item("Photos", True, sub="2 devices · read & write"),
        side_item("Projects", sub="All members · read only"),
        side_item("Archive", sub="Desk PC · upload only"),
        gap=6, sizingH="fixed", w=300,
    )
    perms = card(
        setting_row("Browse", "Inherit · Allow"), divider(16),
        setting_row("Download", "Inherit · Allow"), divider(16),
        setting_row("Upload", "Allow", inherited=False), divider(16),
        setting_row("Delete", "Inherit · Trash"), divider(16),
        setting_row("Conflicts", "Replace, keep backup", inherited=False), divider(16),
        setting_row("Backups retention", "Inherit · Until cleared"),
    )
    access = card(
        list_row("All group members", None, lead=radio(False), pad_x=16), divider(52),
        list_row("Selected devices", None, lead=radio(True), pad_x=16,
                 trailing=row("Devices", chip("Dmytro's Laptop"), chip("Desk PC"),
                              T("+ Add", 13, "Medium", MUTE), gap=6, sizingH="hug")),
    )
    editor = col(
        "Editor",
        row("Head", col("Text", T("Photos", 20, "Bold"),
                        T("C:\\Users\\me\\Pictures", 13, color=MUTE), gap=2), spacer(),
            button("Change folder", primary=False, small=True),
            button("Remove", primary=False, small=True), gap=8),
        T("WHO CAN ACCESS", 11, "Semi Bold", MUTE, letterSpacing=0.6), access,
        row("Perm head", T("PERMISSIONS", 11, "Semi Bold", MUTE, letterSpacing=0.6), spacer(),
            T("Unchanged values follow App defaults", 12, color=MUTE)),
        perms,
        row("Backups", T("3 backup sets · 412 MB", 13, "Medium"), spacer(),
            link(button("Manage backups", primary=False, small=True), "d-backups")),
        gap=12, sizingV="hug",
    )
    return window("Desktop / 04 Settings — shared folders", "settings", main_area(
        T("Settings", 24, "Bold"),
        tabs(*SETTINGS_TABS, active=1, links=SETTINGS_LINKS),
        row("Split", shares, editor, gap=32, align="min"),
    ))


def d_join(err=False):
    step = lambda n, title, sub: row(  # noqa: E731
        f"Step {n}", F("Number", T(str(n), 13, "Bold", SURF), layout="horizontal",
                       mainAlign="center", crossAlign="center", w=28, h=28, sizingH="fixed",
                       sizingV="fixed", cornerRadius=14, fills=[DARK]),
        col("Text", T(title, 14, "Semi Bold"), T(sub, 13, color=MUTE, wrap=True), gap=2),
        align="min")
    info = col(
        "How it works", icon(48, 14, DARK, "Logo"), T("Welcome to LocalDrop", 24, "Bold"),
        T("Send files between your own devices on this network. No account, no cloud.", 14,
          color=MUTE, wrap=True),
        step(1, "Same name and password everywhere",
             "The first device creates the group, the others join it the same way."),
        step(2, "Compare the fingerprint", "The 4+4 code must match on every device."),
        step(3, "Trust your home network", "Your devices find each other only on networks "
             "you trust. Other devices on the network can't see them."),
        gap=16, pad=32, sizingH="fixed", w=400, sizingV="fill", fills=[BG],
    )
    form = col(
        "Form", T("Join or create your group", 20, "Bold"),
        field("Group name", "home"),
        *([join_banner("Windows Firewall blocks LocalDrop", "Allow it on private networks, "
                       "or your devices can't find this PC.", "Allow…")] if err else []),
        err_field("Password", "sunny1", "Too short: 6 characters. Use at least 10, or "
                  "Generate a passphrase.", trailing="Generate") if err else
        field("Password", "••••••••••••", hint="At least 10 characters, or use a generated "
              "passphrase.", trailing="Generate"),
        field("This device's name", "Dmytro's Laptop"),
        trust_row(),
        row("Fingerprint", col("Text", T("Group fingerprint", 13, "Medium", MUTE),
                               T("3fa2-91c0", 22, "Bold", letterSpacing=1)),
            T("Copy", 13, "Medium", MUTE), pad=16, fills=[BG], cornerRadius=12),
        row("Advanced", T("Advanced: listening port, receive folder", 13, "Medium", MUTE),
            spacer(), T("›", 14, color=MUTE)),
        button("Join group", full=True) if err else link(button("Join group", full=True),
                                                         "d-looking"),
        gap=14 if err else 16, pad=32, sizingH="fixed", w=440, sizingV="fill",
    )
    dialog = row("Dialog", info, form, gap=0, align="min", sizingH="hug", sizingV="fixed",
                 h=660, fills=[SURF],
                 cornerRadius=20, clip=True, strokes=[LINE], strokeWeight=1,
                 effects=[{"type": "dropShadow", "color": "#00000014", "offset": [0, 12],
                           "radius": 32}])
    title = row("Title bar", R(12, 12, FAINT, 6, name="Close"), R(12, 12, FAINT, 6, name="Min"),
                R(12, 12, FAINT, 6, name="Max"), spacer(), T("LocalDrop", 12, "Medium", MUTE),
                spacer(), R(52, 12, BLOCK, name="Balance"), gap=8, pad=[10, 14, 10, 14],
                fills=[BLOCK])
    body = col("Body", dialog, gap=0, sizingV="fill", mainAlign="center", crossAlign="center",
               pad=40)
    return F("Desktop / 00 First run — join group", title, body, w=DESK[0], h=DESK[1],
             layout="vertical", gap=0, fills=[BG], clip=True, cornerRadius=12, strokes=[LINE],
             strokeWeight=1)


# ---------------------------------------------------------------- transfers (desktop)
def pair_group(title, state, *jobs, to=None, actions=()):
    head = row("Pair head", T(title, 14, "Semi Bold"), chip(state), spacer(),
               *[T(a, 12, "Medium", MUTE) for a in actions], gap=8, pad=[12, 16, 4, 16])
    head = link(head, to) if to else head
    kids = [head]
    for i, j in enumerate(jobs):
        if i:
            kids.append(divider(16))
        kids.append(j)
    return card(*kids)


def done_row(name, sub, state):
    return row(f"Done / {name}", icon(24, 6), col("Text", T(name, 13, "Medium"),
               T(sub, 12, color=MUTE), gap=1), chip(state), T("›", 14, color=FAINT),
               pad=[8, 16, 8, 16])


def d_transfers(name="Desktop / 06 Transfers"):
    """Transfers: one queue per device pair and direction, one file in flight per queue."""
    out = col(
        "Outgoing",
        T("OUTGOING", 11, "Semi Bold", MUTE, letterSpacing=0.6),
        pair_group("To Dmytro's Laptop", "Sending",
                   link(job("Holiday 2024", "Folder · 412 of 1 820 files · now: bundle 5 of 18",
                            0.34, meta="1.1 of 3.1 GB · 11.4 MB/s · 3 min left", action="×"),
                        "d-details"),
                   job("contract.pdf", "2.4 MB → Inbox", meta="Next · drag to reorder",
                       action=link(T("⋮", 18, "Medium", MUTE, name="More"), "d-job-menu")),
                   actions=("Pause",)),
        pair_group("To iPad", "Offline · waiting",
                   job("Photos for mom", "48 files · 210 MB → Inbox", meta="Queued 2 h ago",
                       action="×"),
                   row("More", T("+ 2 more in queue", 12, "Medium", MUTE), spacer(),
                       T("Open queue  ›", 12, "Semi Bold"), pad=[4, 16, 12, 16]),
                   to="d-offline"),
        gap=10, sizingH="fill",
    )
    inc = col(
        "Incoming",
        T("INCOMING", 11, "Semi Bold", MUTE, letterSpacing=0.6),
        pair_group("From Desk PC", "Receiving",
                   job("Backup.zip", "18.0 GB → Inbox · large file, 64 MB segments", 0.72,
                       meta="13.0 of 18.0 GB · 11.6 MB/s · resumes if interrupted",
                       action="×")),
        pair_group("From Work MacBook", "Waiting for you",
                   job("Docs", "242 files → Projects · 12 already exist",
                       state="Needs decision",
                       action=link(button("Resolve", small=True), "d-conflict"),
                       meta="Asked 5 min ago · either side can answer")),
        gap=10, sizingH="fill",
    )
    finished = card(
        row("Card head", T("Finished today", 14, "Semi Bold"), spacer(),
            T("Clear finished", 12, "Medium", MUTE), pad=[12, 16, 4, 16]),
        done_row("Notes.md", "From Dmytro's Laptop · 4 KB · 09:12", "Done"),
        done_row("Screenshots", "To Desk PC · 36 files · 08:40", "Done"),
        done_row("old-build.iso", "To Work MacBook · cancelled at 41%", "Cancelled"),
    )
    toolbar = row("Toolbar", side_toggle(True), T("Transfers", 22, "Bold"),
                  T("2 active · 4 waiting", 13, color=MUTE), spacer(),
                  disc_pill(), seg("All", "Outgoing", "Incoming", active=0, links={}),
                  button("Pause all", primary=False, small=True), gap=12)
    return window(name, "transfers", main_area(
        toolbar,
        row("Columns", out, inc, gap=16, align="min"),
        finished, gap=16,
    ))


def conflict_file(name, old, new):
    cell = lambda t, w: F("Cell", T(t, 12, color=MUTE), layout="horizontal",  # noqa: E731
                          sizingH="fixed", sizingV="hug", w=w)
    return row(f"Conflict / {name}", icon(20, 5), T(name, 13, "Medium"), spacer(),
               cell(old, 120), cell(new, 120), gap=10, pad=[8, 14, 8, 14])


def d_conflict():
    """Conflict dialog over the Transfers screen (sender or receiver; first answer wins)."""
    left = col(
        "Choice",
        col("Head", T("12 files already exist", 22, "Bold"),
            T("Docs from Work MacBook → Projects. The 230 new files are added either way.", 14,
              color=MUTE, wrap=True), gap=6),
        col("Options",
            option("Upload only new files", "Keep the 12 existing files as they are.",
                   selected=True),
            option("Replace, keep a backup", "Old versions go to Settings › Backups."),
            option("Replace conflicting files", "Old versions are moved to the Trash."),
            link(option("Replace the whole folder", "Also removes files that aren't in this "
                        "upload (Trash)."), "d-mirror"),
            gap=8),
        row("Remember", checkbox(False), T("Always do this for Projects (sets the folder's "
            "conflict mode to Auto)", 13, color=INK, wrap=True), align="min", gap=10),
        gap=18, pad=28, sizingH="fixed", w=440, sizingV="fill",
    )
    files = col(
        "Files",
        row("Head", T("Conflicting files", 14, "Semi Bold"), spacer(),
            T("12", 13, "Medium", MUTE), pad=[0, 0, 6, 0]),
        F("Table", row("Cols", T("NAME", 11, "Semi Bold", MUTE, letterSpacing=0.6), spacer(),
                       F("C1", T("ON THIS PC", 11, "Semi Bold", MUTE, letterSpacing=0.6),
                         layout="horizontal", sizingH="fixed", sizingV="hug", w=120),
                       F("C2", T("INCOMING", 11, "Semi Bold", MUTE, letterSpacing=0.6),
                         layout="horizontal", sizingH="fixed", sizingV="hug", w=120),
                       gap=10, pad=[8, 14, 8, 14], fills=[BG]),
          conflict_file("plan.docx", "84 KB · 2 Mar", "91 KB · today"),
          conflict_file("budget.xlsx", "40 KB · 1 Mar", "44 KB · today"),
          conflict_file("notes.md", "3 KB · 28 Feb", "3 KB · today"),
          conflict_file("logo.svg", "12 KB · 10 Jan", "12 KB · today"),
          conflict_file("spec.pdf", "2.1 MB · 5 Jan", "2.3 MB · today"),
          row("More", T("+ 7 more", 12, "Medium", MUTE), pad=[8, 14, 8, 14]),
          layout="vertical", gap=0, sizingH="fill", sizingV="hug", fills=[SURF],
          cornerRadius=10, clip=True, strokes=[LINE], strokeWeight=1),
        F("Fill", layout="vertical", sizingH="fill", sizingV="fill"),
        T("Work MacBook sees this question too. Whoever answers first decides.", 12,
          color=MUTE, wrap=True),
        row("Actions", spacer(), link(button("Cancel transfer", primary=False), "back"),
            link(button("Continue"), "back"), gap=10),
        gap=10, pad=28, sizingV="fill", fills=[BG],
    )
    dialog = row("Dialog", left, files, gap=0, align="min", sizingH="fixed", w=960,
                 sizingV="fixed", h=600, fills=[SURF], cornerRadius=18, clip=True,
                 effects=[{"type": "dropShadow", "color": "#00000033", "offset": [0, 16],
                           "radius": 40}])
    scrim = F("Scrim", dialog, layout="vertical", mainAlign="center", crossAlign="center",
              w=DESK[0], h=DESK[1], sizingH="fixed", sizingV="fixed", absolute=True,
              fills=["#18181B66"])
    win = d_transfers("Desktop / 07 Conflict dialog")
    win["kids"].append(scrim)
    return win


# ---------------------------------------------------------------- discovery off
OFF_TEXT = ("This PC isn't looking for your devices and they can't find it. Queued files "
            "wait until you turn it back on. Nothing is sent or received meanwhile.")


def d_disc_off():
    toolbar = row("Toolbar", side_toggle(True), nav_btns(), crumbs("Dmytro's Laptop"),
                  spacer(), disc_pill(False), search(), seg("Remote", "Split", active=0),
                  gap=12)
    paused = row("Transfer bar", icon(28, 8), col("Text", T("Transfers paused", 13,
                 "Semi Bold"), T("1 sending · 2 queued · resumes when discovery is on", 12,
                 color=MUTE), gap=2), link(T("Transfers  ›", 13, "Semi Bold"), "d-transfers"),
                 gap=14, pad=[10, 16, 10, 16], fills=[SURF], cornerRadius=12,
                 strokes=[LINE], strokeWeight=1)
    return window("Desktop / 11 Discovery off", "laptop", main_area(
        toolbar, fill_v("Top"),
        row("Center", F("Box", off_state("Discovery is off", OFF_TEXT, "d-remote"),
                        layout="vertical", sizingH="fixed", w=460, sizingV="hug"),
            mainAlign="center"),
        fill_v("Bottom"), paused, gap=16,
    ), disc=False)


def p_disc_off():
    return phone(
        "Phone / 16 Discovery off",
        panel_tabs(1, online=False, peer_sub="Right · not searching", disc=False),
        inset(off_state("Discovery is off", "Not looking for your devices, and they can't "
                        "find this phone. Queued files wait.",
                        "files-root", phone=True), pad=(24, 16, 0, 16)),
        footer=row("Paused", icon(24, 7), T("Transfers paused · 2 queued", 13, "Medium"),
                   spacer(), link(T("View", 13, "Semi Bold"), "transfers"),
                   pad=[10, 16, 10, 16], fills=[SURF], strokes=[LINE], strokeWeight=1,
                   strokeAlign="inside"),
        nav="Files",
    )


# ---------------------------------------------------------------- networks
def net_row(title, sub, state, lead=None, dim=False, to=None, pad_x=16):
    r = list_row(title, sub, lead=lead or icon(28, 8, BLOCK), trailing=state, pad_x=pad_x)
    if dim:
        r["props"]["opacity"] = 0.55
    return link(r, to) if to else r


def trust_btn(to=None):
    b = button("Trust", primary=False, small=True)
    return link(b, to) if to else b


def d_net_menu():
    """Network menu opened from the discovery pill (two networks at once)."""
    pop = col(
        "Network menu",
        row("Head", T("Search on", 14, "Semi Bold"), spacer(),
            T("Wi-Fi + Ethernet connected", 12, color=MUTE), pad=[14, 16, 6, 16]),
        list_row("Trusted networks", "Now: Home", lead=radio(True), pad_x=16),
        divider(52),
        list_row("Only Home", "Wi-Fi · 192.168.1.23/24", lead=radio(False), pad_x=16),
        R(10, 1, LINE, name="Line", sizingH="fill"),
        row("Now", T("CONNECTED", 11, "Semi Bold", MUTE, letterSpacing=0.6),
            pad=[10, 16, 2, 16]),
        net_row("Home", "Wi-Fi · trusted", chip("Discovering", True)),
        net_row("Office LAN", "Ethernet · not trusted", trust_btn("d-networks")),
        net_row("vEthernet (WSL)", "Virtual adapter", chip("Ignored"), dim=True),
        R(10, 1, LINE, name="Line", sizingH="fill"),
        link(row("Settings link", T("Network settings", 13, "Semi Bold"), spacer(),
                 T("›", 14, color=MUTE), pad=[12, 16, 12, 16]), "d-networks"),
        gap=0, sizingH="fixed", w=380, fills=[SURF], cornerRadius=14, clip=True,
        strokes=[LINE], strokeWeight=1,
        effects=[{"type": "dropShadow", "color": "#0000001F", "offset": [0, 10],
                  "radius": 28}])
    layer = link(F("Popover layer", pop, layout="vertical", crossAlign="min",
                   padding=[96, 0, 0, 640], w=DESK[0], h=DESK[1], sizingH="fixed",
                   sizingV="fixed", absolute=True, fills=["#18181B14"]), "back")
    win = d_remote("Desktop / 12 Network menu")
    win["kids"].append(layer)
    return win


def d_new_net():
    """A network that isn't trusted: discovery stays off; ask once whether to trust it."""
    toolbar = row("Toolbar", side_toggle(True), nav_btns(), crumbs("Dmytro's Laptop"),
                  spacer(), disc_pill(True, net="no trusted network"), search(),
                  seg("Remote", "Split", active=0), gap=12)
    ask = col(
        "New network", icon(48, 14, BLOCK), T("Network not trusted", 13, "Semi Bold", MUTE),
        T("Trust “Office-5G”?", 20, "Bold"),
        T("LocalDrop only searches on networks you trust. Here it stays silent: your devices "
          "can't see this PC and you can't see them.", 13, color=MUTE, wrap=True,
          textAlignHorizontal="center"),
        T("Wi-Fi · 10.20.0.0/22 · gateway 10.20.0.1", 12, color=MUTE),
        row("Answers", link(button("Never ask", primary=False), "d-disc-off"),
            link(button("Not now", primary=False), "d-disc-off"),
            link(button("Trust this network"), "d-remote"), gap=10, sizingH="hug"),
        T("Change it later in Settings › Networks.", 12, color=MUTE),
        gap=12, pad=32, crossAlign="center", fills=[SURF], cornerRadius=16)
    note = row("Notification", icon(20, 6), T("A system notification asks the same when "
               "the window is closed.", 12, color=MUTE), pad=[10, 14, 10, 14],
               cornerRadius=10, strokes=[LINE], strokeWeight=1, sizingH="hug")
    return window("Desktop / 13 New network", "laptop", main_area(
        toolbar, fill_v("Top"),
        row("Center", F("Box", ask, layout="vertical", sizingH="fixed", w=540,
                        sizingV="hug"), mainAlign="center"),
        row("Note row", note, mainAlign="center"),
        fill_v("Bottom"), gap=16,
    ), disc=False)


def d_networks():
    w = (110, 120)
    now = scard(
        net_row("Home", "Wi-Fi · 192.168.1.23/24 · trusted", chip("Discovering", True)),
        net_row("Office LAN", "Ethernet · 10.0.5.17/22 · not trusted", trust_btn()),
        net_row("vEthernet (WSL)", "Virtual · 172.29.80.1/20", row("Inc", T("Include", 12,
                "Medium", MUTE), small_toggle(False), gap=6, sizingH="hug"), dim=True),
    )
    behaviour = scard(
        srow("Discovery at launch", "Turn discovery on when LocalDrop opens", toggle(True)),
        srow("On a new network", "Ask to trust · Stay silent", select_box("Ask to trust")),
        srow("Trust every network", "Not recommended: announces this PC on any Wi-Fi",
             toggle(False)),
        srow("Show Wi-Fi names", "Windows only gives them with Location turned on",
             button("Turn on…", primary=False, small=True)),
    )

    def known(name, kind, ident, rule, seen):
        return frow(name, (ident, select_box(rule)), w, lead=icon(24, 6),
                    sub=f"{kind} · {seen}", trailing="⋮", box=False)

    tbl = card(
        frow("Network", ("Recognised by", "Trust"), w, header=True, box=False),
        divider(0),
        known("Home", "Wi-Fi", "Router address", "Trusted", "now"),
        divider(16),
        known("Office LAN", "Ethernet", "Router address", "Not trusted", "now"),
        divider(16),
        known("Office-5G", "Wi-Fi", "Subnet only", "Never ask", "yesterday"),
        divider(16),
        known("Parents' Wi-Fi", "Wi-Fi", "Router address", "Trusted", "3 weeks ago"),
    )
    return settings_window(
        "Desktop / 14 Settings — networks", 4,
        row("Columns",
            col("Left", slabel("Connected now"), now, slabel("Discovery"), behaviour,
                gap=10),
            col("Right", slabel("Remembered networks", "⋮ Rename · Forget"), tbl,
                T("No network is trusted until you say so. Networks are recognised on this "
                  "PC only, by the router's hardware address (or the Wi-Fi name if "
                  "allowed); nothing about them is sent to other devices.", 12, color=MUTE,
                  wrap=True), gap=10),
            gap=24, align="min"),
    )


def p_new_net():
    body = sheet(
        icon(44, 12, BLOCK),
        T("Network not trusted", 13, "Semi Bold", MUTE),
        T("Trust “Office-5G”?", 20, "Bold"),
        T("LocalDrop only searches on networks you trust. Wi-Fi · 10.20.0.x", 14,
          color=MUTE, wrap=True, textAlignHorizontal="center"),
        col("Answers", link(button("Trust this network", full=True), "files-root"),
            link(button("Not now", primary=False, full=True), "disc-off"),
            link(button("Never ask", primary=False, full=True), "disc-off"), gap=8),
        T("Change it later in Settings › Networks.", 12, color=MUTE),
    )
    dim = F("Scrim", w=PHONE[0], h=10, sizingH="fill", sizingV="fill", fills=["#18181B66"])
    return F("Phone / 17 New network", status_bar(), dim, body, w=PHONE[0], h=PHONE[1],
             layout="vertical", gap=0, fills=[BLOCK], clip=True, cornerRadius=28)


def p_networks():
    now = card(net_row("Home", "Wi-Fi · 192.168.1.42 · trusted", chip("On", True)))
    remembered = card(
        net_row("Home", "Wi-Fi · recognised by name", select_box("Trusted")),
        divider(60),
        net_row("Office-5G", "Wi-Fi · recognised by name", select_box("Never ask")),
        divider(60),
        net_row("Parents' Wi-Fi", "Wi-Fi · subnet only", select_box("Not trusted")),
    )
    rows = card(
        list_row("Discovery at launch", None, lead=None, trailing=toggle(True), pad_x=16),
        divider(16),
        list_row("On a new network", None, lead=None, trailing=select_box("Ask to trust"),
                 pad_x=16),
        divider(16),
        list_row("Trust every network", "Not recommended", lead=None, trailing=toggle(False),
                 pad_x=16),
        divider(16),
        list_row("Use Wi-Fi names", "Needs location access", lead=None,
                 trailing=toggle(True), pad_x=16),
    )
    return phone(
        "Phone / 18 Networks",
        app_bar("Networks", "Discovery runs only on trusted networks", back=True),
        section("Connected now"), inset(now),
        section("Discovery"), inset(rows),
        section("Remembered", "Tap to rename or forget"), inset(remembered),
        nav="Settings",
    )


# ---------------------------------------------------------------- settings tabs (desktop)
def srow(title, sub=None, control=None):
    """Settings row: title / sub on the left, a control on the right."""
    if isinstance(control, str):
        control = T(control, 13, "Medium", MUTE)
    return list_row(title, sub, lead=None, trailing=control, pad_x=16)


def scard(*rows):
    kids = []
    for i, r in enumerate(rows):
        if i:
            kids.append(divider(16))
        kids.append(r)
    return card(*kids)


def slabel(text, trailing=None):
    return row(f"Label / {text}", T(text.upper(), 11, "Semi Bold", MUTE, letterSpacing=0.6),
               spacer(), *( [T(trailing, 12, color=MUTE)] if trailing else []))


def settings_window(name, active, *body):
    return window(name, "settings", main_area(
        T("Settings", 24, "Bold"),
        tabs(*SETTINGS_TABS, active=active, links=SETTINGS_LINKS),
        *body, gap=16))


def d_general():
    group = scard(
        row("Group", col("Text", T("Group", 13, "Medium", MUTE), T("home", 18, "Semi Bold"),
                         gap=2),
            col("Text", T("Fingerprint", 13, "Medium", MUTE),
                T("3fa2-91c0", 18, "Bold", letterSpacing=1), gap=2),
            T("Copy", 13, "Semi Bold"), pad=[14, 16, 14, 16], gap=24),
        srow("Change password", "Devices without the new one drop out",
             link(button("Change…", primary=False, small=True), "d-change-pass")),
        srow("Leave group", "Forget the group key on this PC. Queues are kept.",
             link(button("Leave", primary=False, small=True), "d-leave")),
    )
    device = scard(
        srow("This device's name", "Shown to your other devices",
             select_box("Dmytro's Laptop")),
        srow("Visible to your group", "Off: you can still send, others can't find this PC",
             toggle(True)),
        srow("Device ID", "Tied to this install's key", T("a91f-07c2-5d1e", 13, "Medium",
                                                          MUTE)),
    )
    receiving = scard(
        srow("Inbox", "C:\\Users\\me\\Downloads\\LocalDrop",
             link(button("Change…", primary=False, small=True), "d-recv")),
        srow("Files from offline queues", "Sent while you were away",
             select_box("Same as Inbox")),
    )
    startup = scard(
        srow("Launch at login", None, toggle(True)),
        srow("Keep running in the tray", "Needed to deliver queued files", toggle(True)),
        srow("Listening port", "Discovery: mDNS + broadcast (47473)", select_box("Auto")),
        srow("Theme", None, select_box("System")),
    )
    return settings_window(
        "Desktop / 08 Settings — general", 0,
        row("Columns",
            col("Left", slabel("Group"), group, slabel("This device"), device, gap=10),
            col("Right", slabel("Receiving"), receiving, slabel("Startup & network"), startup,
                gap=10),
            gap=24, align="min"),
    )


def d_defaults():
    accept = card(
        list_row("Automatically from group members", "Needed for sending ahead while you're "
                 "away", lead=radio(True), pad_x=16),
        divider(52),
        list_row("Ask me every time", "Jobs wait for you; the sender's queue keeps going",
                 lead=radio(False), pad_x=16),
        divider(52),
        link(row("Exceptions", T("Per-device exceptions: Work MacBook asks", 13, color=MUTE),
                 spacer(), T("Devices  ›", 13, "Semi Bold"), pad=[12, 16, 12, 52]),
             "d-devices"),
    )
    perms = scard(
        srow("Browse", "Other devices can list files", select_box("Allow")),
        srow("Download", None, select_box("Allow")),
        srow("Upload", None, select_box("Deny")),
        srow("Delete by others", "Deny · Move to Trash · Delete permanently",
             select_box("Move to Trash")),
        srow("Conflicts", "Ask, or always use one mode", select_box("Ask")),
    )
    backups = scard(
        srow("Keep backups", None, select_box("Until cleared")),
        srow("Max size per folder", None, select_box("No limit")),
    )
    sending = scard(
        srow("Offline queue", "Point to the original files; warn if they change",
             select_box("Reference files")),
    )
    return settings_window(
        "Desktop / 09 Settings — app defaults", 2,
        T("Every shared folder uses these unless it changes them.", 13, color=MUTE),
        row("Columns",
            col("Left", slabel("Accepting files"), accept, slabel("Sending"), sending,
                gap=10),
            col("Right", slabel("Shared folder permissions"), perms, slabel("Backups"),
                backups, gap=10),
            gap=24, align="min"),
    )


def d_devices():
    w = (130, 150, 150, 70)

    def dev(name, plat, seen, accept, queue, online, sub=None):
        lead = F("Avatar", R(28, 28, BLOCK, 14), status_dot(online), layout="horizontal",
                 gap=-8, crossAlign="max", sizingH="hug", sizingV="hug")
        return link(frow(name, (plat, seen, select_box(accept), queue), w, lead=lead,
                         sub=sub, trailing="⋮", box=False), "d-device")

    tbl = card(
        frow("Device", ("Platform", "Last seen", "Accept files", "Queue"), w, header=True,
             box=False),
        divider(0),
        frow("Dmytro's Laptop (this PC)", ("Windows", "—", "—", "—"), w, box=False,
             lead=icon(28, 14, DARK), sub="a91f-07c2-5d1e", trailing="⋮"),
        divider(16),
        dev("Desk PC", "Linux", "Online", "Default · Auto", "—", True, "3c07-118a-e4f0"),
        divider(16),
        dev("iPad", "iPadOS", "2 h ago", "Default · Auto", "3", False, "77b2-c91d-0a3e"),
        divider(16),
        dev("Work MacBook", "macOS", "Yesterday", "Ask", "—", False, "e012-6f4b-93cc"),
    )
    menu_hint = row("Menu", T("⋮  Rename · Open queue · Forget device", 12, color=MUTE),
                    pad=[0, 4, 0, 4])
    note = row("Note", icon(20, 6), col("Text",
               T("Forgetting a device doesn't lock it out", 14, "Semi Bold"),
               T("Anyone with the group password can rejoin. To remove a device for good, "
                 "change the password on all your other devices (General › Change password).",
                 13, color=MUTE, wrap=True), gap=4),
               pad=16, align="min", cornerRadius=12, strokes=[LINE], strokeWeight=1)
    warn = link(row("Key warning", icon(20, 6, DARK), col("Text",
               T("If a device's key changes, it's blocked until you confirm it here.", 13,
                 "Medium"),
               T("This happens after a reinstall, or if someone imitates the device.", 12,
                 color=MUTE, wrap=True), gap=2),
               pad=14, align="min", cornerRadius=12, fills=[SURF]), "d-key")
    return settings_window(
        "Desktop / 10 Settings — devices", 3,
        row("Head", T("Devices that joined your group and were seen by this PC.", 13,
                      color=MUTE), spacer(), T("4 known", 13, "Medium", MUTE)),
        tbl, menu_hint, warn, note,
    )


# ---------------------------------------------------------------- group dialogs
def modal(win, name, dialog):
    """Put a dialog over a desktop window (dimmed)."""
    win["name"] = name
    win["kids"].append(F("Scrim", dialog, layout="vertical", mainAlign="center",
                         crossAlign="center", w=DESK[0], h=DESK[1], sizingH="fixed",
                         sizingV="fixed", absolute=True, fills=["#18181B66"]))
    return win


def dialog_box(*kids, w=520):
    return col("Dialog", *kids, gap=16, pad=28, sizingH="fixed", w=w, fills=[SURF],
               cornerRadius=18,
               effects=[{"type": "dropShadow", "color": "#00000033", "offset": [0, 16],
                         "radius": 40}])


def fp_change():
    box = lambda label, v, on: col(label, T(label, 12, "Medium", MUTE),  # noqa: E731
                                   T(v, 20, "Bold", INK if on else FAINT, letterSpacing=1),
                                   gap=2, sizingH="hug")
    return row("Fingerprint change", box("Current fingerprint", "3fa2-91c0", False),
               T("→", 18, "Medium", MUTE), box("New fingerprint", "7c41-0b9e", True),
               spacer(), T("Copy", 13, "Semi Bold"), gap=16, pad=16, fills=[BG],
               cornerRadius=12)


def info_line(text):
    return row("Info", R(6, 6, MUTE, 3, name="Bullet"), T(text, 13, color=MUTE, wrap=True),
               gap=10, align="center")


def check_opt(title, sub, checked):
    return row(f"Option / {title}", checkbox(checked),
               col("Text", T(title, 14, "Medium"), T(sub, 12, color=MUTE, wrap=True), gap=2),
               align="min", gap=10)


CHANGE_PASS_TEXT = ("A new password makes a new group key. Devices that don't get it drop out "
                    "until they join again with it.")
LEAVE_TEXT = ("This device forgets the group key and stops finding your devices. Files on "
              "disk aren't touched.")


def d_change_pass():
    dlg = dialog_box(
        T("Change group password", 20, "Bold"),
        T(CHANGE_PASS_TEXT, 14, color=MUTE, wrap=True),
        field("Current password", "••••••••••••", hint="Checked on this PC; it isn't stored "
              "anywhere."),
        field("New password", "maple-orbit-fence-tulip", hint="Strong · generated 4-word "
              "passphrase", trailing="Generate"),
        fp_change(),
        col("Notes",
            info_line("Then enter the new password on each device you keep."),
            info_line("Lost or unwanted device? Just don't give it the new password."),
            info_line("Queues and known devices are kept; each device reconnects once it "
                      "has the new password."),
            gap=8),
        row("Actions", spacer(), link(button("Cancel", primary=False), "back"),
            link(button("Change password"), "back"), gap=10),
        w=560)
    return modal(d_general(), "Desktop / 15 Change password", dlg)


def d_leave():
    dlg = dialog_box(
        T("Leave group “home”?", 20, "Bold"),
        T(LEAVE_TEXT, 14, color=MUTE, wrap=True),
        col("Options",
            check_opt("Keep the send queue", "3 jobs to iPad · sent if you join this group "
                      "again", True),
            check_opt("Forget known devices", "Otherwise the 4 devices are recognised again "
                      "if you rejoin", False),
            gap=12),
        T("Shared folders, backups and trusted networks stay as they are.", 12, color=MUTE,
          wrap=True),
        row("Actions", spacer(), link(button("Cancel", primary=False), "back"),
            link(button("Leave group"), "d-join"), gap=10),
        w=480)
    return modal(d_general(), "Desktop / 16 Leave group", dlg)


def p_change_pass():
    return phone(
        "Phone / 19 Change password",
        app_bar("Change password", "Group home", back=True),
        inset(T(CHANGE_PASS_TEXT, 14, color=MUTE, wrap=True), pad=(0, 20, 12, 20)),
        col("Form",
            field("Current password", "••••••••••••"),
            field("New password", "maple-orbit-fence-tulip", hint="Strong · generated "
                  "passphrase", trailing="Generate"),
            gap=14, pad=[0, 20, 0, 20]),
        inset(row("Fingerprint", col("Text", T("New fingerprint", 12, "Medium", MUTE),
                                     T("7c41-0b9e", 22, "Bold", letterSpacing=1), gap=2),
                  T("was 3fa2-91c0", 12, color=FAINT), pad=16, fills=[SURF],
                  cornerRadius=12), pad=(16, 16, 0, 16)),
        inset(col("Notes",
                  info_line("Then enter it on each device you keep."),
                  info_line("Lost device? Just don't give it the new password."),
                  gap=8), pad=(14, 20, 0, 20)),
        footer=col("Footer", link(button("Change password", full=True), "back"), gap=0,
                   pad=[12, 20, 30, 20], fills=[SURF], strokes=[LINE], strokeWeight=1,
                   strokeAlign="inside"),
    )


def p_leave():
    body = sheet(
        T("Leave group “home”?", 20, "Bold"),
        T(LEAVE_TEXT, 14, color=MUTE, wrap=True, textAlignHorizontal="center"),
        col("Options",
            check_opt("Keep the send queue", "3 jobs to iPad · sent if you rejoin", True),
            check_opt("Forget known devices", "Otherwise recognised again if you rejoin",
                      False),
            gap=12),
        T("Shared folders, backups and trusted networks stay.", 12, color=MUTE),
        col("Actions", link(button("Leave group", full=True), "join"),
            link(button("Cancel", primary=False, full=True), "back"), gap=8),
    )
    dim = F("Scrim", w=PHONE[0], h=10, sizingH="fill", sizingV="fill", fills=["#18181B66"])
    return F("Phone / 20 Leave group", status_bar(), dim, body, w=PHONE[0], h=PHONE[1],
             layout="vertical", gap=0, fills=[BLOCK], clip=True, cornerRadius=28)


# ---------------------------------------------------------------- P1 flows
def psheet(name, *kids):
    """Phone bottom sheet over a dimmed screen."""
    dim = F("Scrim", w=PHONE[0], h=10, sizingH="fill", sizingV="fill", fills=["#18181B66"])
    return F(name, status_bar(), dim, sheet(*kids), w=PHONE[0], h=PHONE[1],
             layout="vertical", gap=0, fills=[BLOCK], clip=True, cornerRadius=28)


def avatar(online=True, size=40):
    return F("Avatar", R(size, size, BLOCK, size // 2), status_dot(online),
             layout="horizontal", gap=-10, crossAlign="max", sizingH="hug", sizingV="hug")


def steps_card(*items):
    kids = []
    for i, (t, sub) in enumerate(items, 1):
        kids.append(row(f"Step {i}", F("Number", T(str(i), 12, "Bold", SURF),
                                       layout="horizontal", mainAlign="center",
                                       crossAlign="center", w=24, h=24, sizingH="fixed",
                                       sizingV="fixed", cornerRadius=12, fills=[DARK]),
                        col("Text", T(t, 14, "Semi Bold"), T(sub, 13, color=MUTE, wrap=True),
                            gap=2), align="min", gap=12))
    return col("Steps", *kids, gap=14)


LOOK_STEPS = (
    ("Open LocalDrop on another device", "Phone, tablet or computer on the same network."),
    ("Join with the same name and password", "Group “home”. The fingerprint must show "
     "3fa2-91c0."),
    ("Trust this network there too", "Both devices search only on networks they trust."),
)


# 1. empty state after joining
def d_looking():
    found = link(row("Found", icon(20, 6, DARK), T("When a device appears it opens here "
                     "automatically.", 13, color=MUTE), spacer(), T("Desk PC found  ›", 13,
                     "Semi Bold"), pad=[10, 14, 10, 14], cornerRadius=10, strokes=[LINE],
                     strokeWeight=1), "d-remote")
    card_ = col(
        "Looking", F("Radar", F("Ring 2", F("Ring 1", icon(28, 14, DARK, "Dot"),
                                            layout="horizontal", mainAlign="center",
                                            crossAlign="center", w=64, h=64, sizingH="fixed",
                                            sizingV="fixed", cornerRadius=32, fills=[BLOCK]),
                                layout="horizontal", mainAlign="center", crossAlign="center",
                                w=104, h=104, sizingH="fixed", sizingV="fixed",
                                cornerRadius=52, strokes=[LINE], strokeWeight=1),
                     layout="horizontal", mainAlign="center", sizingH="fill", sizingV="hug"),
        col("Head", T("Looking for your devices…", 22, "Bold"),
            T("Searching on Home (Wi-Fi, trusted). Nothing found yet.", 14, color=MUTE),
            gap=4, crossAlign="center"),
        steps_card(*LOOK_STEPS),
        row("More", T("Not showing up?", 13, "Semi Bold"), T("Check that both devices are on "
            "the same Wi-Fi and the firewall allows LocalDrop.", 13, color=MUTE, wrap=True),
            align="min", gap=8),
        link(T("Connect by address…", 13, "Semi Bold", name="Connect by address"), "d-connect"),
        gap=22, pad=32, sizingH="fixed", w=560, fills=[SURF], cornerRadius=16)
    toolbar = row("Toolbar", side_toggle(True), T("LocalDrop", 15, "Semi Bold"), spacer(),
                  disc_pill(), search(), seg("Remote", "Split", active=0), gap=12)
    return window("Desktop / 17 Looking for devices", "none", main_area(
        toolbar, fill_v("Top"), row("Center", card_, mainAlign="center"),
        row("Found row", found, mainAlign="center"), fill_v("Bottom"), gap=16,
    ), disc="empty")


def p_looking():
    return phone(
        "Phone / 21 Looking for devices",
        panel_tabs(1, peer="No device", peer_sub="Right · searching…"),
        col("Looking", F("Radar", icon(28, 14, DARK, "Dot"), layout="horizontal",
                         mainAlign="center", crossAlign="center", w=88, h=88,
                         sizingH="fixed", sizingV="fixed", cornerRadius=44, fills=[BLOCK]),
            T("Looking for your devices…", 20, "Bold"),
            T("Searching on Home (Wi-Fi, trusted).", 13, color=MUTE),
            gap=10, pad=[28, 20, 16, 20], crossAlign="center"),
        inset(card(col("Steps wrap", steps_card(*LOOK_STEPS), pad=16)), pad=(0, 16, 0, 16)),
        inset(link(row("Found", T("Desk PC found", 14, "Semi Bold"), spacer(),
                       T("Open  ›", 13, "Semi Bold"), pad=14, fills=[SURF], cornerRadius=12),
                   "files-root"), pad=(12, 16, 0, 16)),
        inset(link(T("Not showing up? Connect by address", 13, "Semi Bold", MUTE,
                     name="Connect by address"), "connect"), pad=(14, 20, 0, 20)),
        nav="Files",
    )


# 2. incoming request (accept policy = Ask)
def incoming_body(phone=False):
    files = card(
        list_row("Contracts", "Folder · 9 files · 840 MB", pad_x=14),
        divider(62),
        list_row("scan_0042.pdf", "3.1 MB", pad_x=14),
        divider(62),
        list_row("+ 2 more", None, lead=None, pad_x=14),
    )
    return [
        row("From", avatar(True), col("Text", T("Work MacBook wants to send you", 13,
                                                color=MUTE),
                                      T("11 items · 1.2 GB", 20, "Bold"), gap=2)),
        files,
        row("Dest", T("Save to", 13, color=MUTE), select_box("Inbox · Downloads/LocalDrop"),
            gap=10),
        check_opt("Always accept from Work MacBook", "Changes this device to Auto in "
                  "Settings › Devices", False),
    ]


def d_incoming():
    dlg = dialog_box(*incoming_body(),
                     T("Waits up to 10 min; the sender's queue goes on meanwhile.", 12,
                       color=MUTE),
                     row("Actions", spacer(), link(button("Decline", primary=False), "back"),
                         link(button("Accept"), "d-transfers"), gap=10), w=520)
    return modal(d_remote(), "Desktop / 18 Incoming request", dlg)


def p_incoming():
    return psheet("Phone / 22 Incoming request", *incoming_body(True),
                  col("Actions", link(button("Accept", full=True), "transfers"),
                      link(button("Decline", primary=False, full=True), "back"), gap=8))


# 3. key changed (pinning mismatch)
def key_body():
    fp = lambda label, v, on: col(label, T(label, 12, "Medium", MUTE),  # noqa: E731
                                  T(v, 15, "Bold", INK if on else FAINT, letterSpacing=1),
                                  gap=2, sizingH="hug")
    return [
        row("Head", icon(40, 12, DARK), col("Text", T("Desk PC's key doesn't match", 20,
                                                       "Bold"),
                                             T("Refused", 13, color=MUTE),
                                             gap=2)),
        # The device id is derived from the key: a reinstalled device shows up as a new
        # device, so a mismatch is never "expected" and there is no "trust new key".
        T("A device in your group answered as Desk PC but couldn't prove it has that "
          "device's key, so LocalDrop didn't connect. A reinstalled device doesn't cause "
          "this: it shows up as a new device. Someone may be pretending to be Desk PC.", 14,
          color=MUTE, wrap=True),
        row("Keys", fp("Known key", "3c07-118a-e4f0", False), T("→", 16, color=MUTE),
            fp("New key", "9b2d-40e7-1c58", True), gap=16, pad=14, fills=[BG],
            cornerRadius=12),
        T("If unsure, check the device ID in Settings › General on Desk PC.", 12,
          color=MUTE, wrap=True),
    ]


def d_key():
    dlg = dialog_box(*key_body(),
                     row("Actions", link(T("Forget Desk PC", 13, "Semi Bold", MUTE), "back"),
                         spacer(), link(button("Keep blocked"), "back"), gap=10), w=540)
    return modal(d_remote(), "Desktop / 19 Key changed", dlg)


def p_key():
    return psheet("Phone / 23 Key changed", *key_body(),
                  col("Actions", link(button("Keep blocked", full=True), "back"),
                      link(button("Forget Desk PC", primary=False, full=True), "back"),
                      gap=8))


# 4. desktop: drop overlay + send dialog
def d_drop():
    def target(name, sub, ok):
        return row(f"Target / {name}", icon(28, 8, DARK if ok else BLOCK),
                   col("Text", T(name, 15, "Semi Bold", INK if ok else FAINT),
                       T(sub, 12, color=MUTE), gap=2),
                   chip("Drop here" if ok else "Read only", ok), gap=12, pad=14,
                   fills=[SURF], cornerRadius=12,
                   strokes=[DARK if ok else LINE], strokeWeight=2 if ok else 1)
    layer = col(
        "Drop layer",
        T("Drop 3 items to send to Dmytro's Laptop", 22, "Bold", SURF),
        T("Holiday 2024, contract.pdf, IMG_0142.MOV · 4.3 GB", 14, color=BLOCK),
        col("Targets", target("Inbox", "Downloads/LocalDrop", True),
            target("Photos", "You can upload · conflicts: ask", True),
            target("Projects", "Browse · download only", False),
            gap=10, sizingH="fixed", w=460),
        T("Drop on the window: Inbox · on a folder: into that folder", 13, color=BLOCK),
        gap=16, crossAlign="center", mainAlign="center", w=DESK[0], h=DESK[1],
        sizingH="fixed", sizingV="fixed", absolute=True, fills=["#18181BB3"])
    win = d_remote()
    win["name"] = "Desktop / 20 Drop to send"
    win["kids"].append(link(layer, "d-send"))
    return win


def d_send():
    items = card(
        list_row("Holiday 2024", "Folder · 1 820 files · 3.1 GB", trailing="×", pad_x=14),
        divider(62), list_row("contract.pdf", "2.4 MB", trailing="×", pad_x=14),
        divider(62), list_row("IMG_0142.MOV", "1.2 GB", trailing="×", pad_x=14),
    )
    dest = card(
        list_row("Inbox", "Downloads/LocalDrop on Dmytro's Laptop", lead=radio(True),
                 pad_x=14),
        divider(48),
        list_row("Shared folder", None, lead=radio(False), pad_x=14,
                 trailing=row("Pick", select_box("Photos"), select_box("/ Camera"), gap=6,
                              sizingH="hug")),
    )
    dlg = dialog_box(
        row("Head", T("Send 3 items", 20, "Bold"), T("4.3 GB", 14, color=MUTE), spacer(),
            T("+ Add files", 13, "Semi Bold"), gap=10),
        items,
        row("To", T("To", 13, "Medium", MUTE), select_box("● Dmytro's Laptop · online"),
            T("Offline devices get it when they're back", 12, color=MUTE), gap=10),
        T("DESTINATION", 11, "Semi Bold", MUTE, letterSpacing=0.6), dest,
        T("Large files go one by one, small ones are bundled. Folders keep their structure.",
          12, color=MUTE, wrap=True),
        row("Actions", spacer(), link(button("Cancel", primary=False), "back"),
            link(button("Send"), "d-transfers"), gap=10),
        w=600)
    return modal(d_remote(), "Desktop / 21 Send files", dlg)


# 5. add shared folder
def share_form(phone=False):
    access = card(
        list_row("All group members", "Including devices that join later",
                 lead=radio(False), pad_x=14),
        divider(48),
        list_row("Selected devices", None, lead=radio(True), pad_x=14,
                 trailing=None if phone else row("Devices", chip("Desk PC", True),
                                                 chip("Dmytro's Laptop"), gap=6,
                                                 sizingH="hug")),
    )
    perms = card(
        default_row("Browse", "Allow"), divider(14),
        default_row("Download", "Allow"), divider(14),
        default_row("Upload", "Allow"), divider(14),
        default_row("Delete by others", "Move to Trash"), divider(14),
        default_row("Conflicts", "Ask"),
    )
    return access, perms


def d_add_share():
    access, perms = share_form()
    left = col("Left",
               field("Folder", "D:\\Work\\Clients", trailing="Choose…"),
               field("Name shown to others", "Clients", hint="The real path is never shown."),
               T("WHO CAN SEE IT", 11, "Semi Bold", MUTE, letterSpacing=0.6), access,
               gap=12)
    right = col("Right",
                row("Perm head", T("PERMISSIONS", 11, "Semi Bold", MUTE, letterSpacing=0.6),
                    spacer(), T("From App defaults · change to override", 12, color=MUTE)),
                perms, gap=12)
    dlg = dialog_box(
        T("Add shared folder", 20, "Bold"),
        row("Columns", left, right, gap=24, align="min"),
        row("Actions", T("Only this folder and what's inside it is visible.", 12,
                         color=MUTE), spacer(),
            link(button("Cancel", primary=False), "back"),
            link(button("Add folder"), "back"), gap=10),
        w=900)
    return modal(d_settings(), "Desktop / 22 Add shared folder", dlg)


def p_add_share():
    access, perms = share_form(phone=True)
    return phone(
        "Phone / 24 Add shared folder",
        app_bar("Add shared folder", back=True),
        col("Form", field("Folder", "Documents/Clients", trailing="Choose"),
            field("Name shown to others", "Clients"), gap=12, pad=[0, 20, 0, 20]),
        section("Who can see it"), inset(access),
        section("Permissions", "From App defaults"), inset(perms),
        footer=col("Footer", link(button("Add folder", full=True), "back"), gap=0,
                   pad=[12, 20, 30, 20], fills=[SURF], strokes=[LINE], strokeWeight=1,
                   strokeAlign="inside"),
    )


# 6. transfer details + errors
def file_state(name, size, state, sub=None, action=None, strong=False):
    return row(f"File / {name}", icon(24, 6),
               col("Text", T(name, 14, "Medium"), *( [T(sub, 12, color=MUTE)] if sub else []),
                   gap=2),
               T(size, 13, color=MUTE), chip(state, strong),
               *( [action] if action else []), gap=12, pad=[9, 16, 9, 16])


def d_details():
    files = card(
        row("Head", T("Files", 14, "Semi Bold"), spacer(),
            row("Filter", chip("All", True), chip("Failed · 2"), chip("Done · 410"), gap=6,
                sizingH="hug"), pad=[12, 16, 6, 16]),
        file_state("IMG_0412.jpg", "4.2 MB", "Sending", "bundle 5 of 18", strong=True),
        divider(56),
        file_state("IMG_0399.jpg", "3.9 MB", "Failed", "Changed on this PC while sending",
                   button("Retry", primary=False, small=True)),
        divider(56),
        file_state("VID_0388.mp4", "1.4 GB", "Failed", "Can't read: in use by another app",
                   button("Retry", primary=False, small=True)),
        divider(56),
        file_state("IMG_0413.jpg", "4.0 MB", "Waiting"),
        divider(56),
        file_state("IMG_0001.jpg", "4.1 MB", "Done"),
    )
    space = row("Space", icon(20, 6, DARK), col("Text",
                T("Dmytro's Laptop is running out of space", 14, "Semi Bold"),
                T("2.0 GB still to send, 1.2 GB free on its Downloads drive. The job pauses "
                  "before the disk is full.", 13, color=MUTE, wrap=True), gap=2),
                button("Pause now", primary=False, small=True), pad=14, align="min",
                cornerRadius=12, fills=[SURF], strokes=[DARK], strokeWeight=1.5)
    head = row("Header", link(T("‹", 26, "Medium", name="Back"), "back"),
               icon(40, 12), col("Text", T("Holiday 2024 → Dmytro's Laptop", 20, "Bold"),
                                 T("Folder · into Inbox · started 09:41", 13, color=MUTE),
                                 gap=2),
               button("Pause", primary=False, small=True),
               button("Cancel job", primary=False, small=True), gap=12)
    stats = row("Stats", *[col(f"Stat {k}", T(k, 12, color=MUTE), T(v, 16, "Semi Bold"), gap=2,
                               sizingH="fill")
                           for k, v in (("Progress", "1.1 of 3.1 GB"), ("Files", "412 of 1 820"),
                                        ("Speed", "11.4 MB/s"), ("Left", "~3 min"))],
                gap=16, pad=16, fills=[SURF], cornerRadius=12)
    return window("Desktop / 23 Transfer details", "transfers", main_area(
        head, col("Progress", progress(0.34), gap=0), stats, space, files, gap=14))


def p_details():
    files = card(
        file_state("IMG_0412.jpg", "4.2 MB", "Sending", strong=True), divider(56),
        file_state("IMG_0399.jpg", "3.9 MB", "Failed", "Changed while sending"), divider(56),
        file_state("VID_0388.mp4", "1.4 GB", "Failed", "Can't read the file"), divider(56),
        file_state("IMG_0413.jpg", "4.0 MB", "Waiting"),
    )
    return phone(
        "Phone / 25 Transfer details",
        app_bar("Holiday 2024", "To Dmytro's Laptop · into Inbox", back=True),
        inset(col("Progress", progress(0.34), row("Meta", T("1.1 of 3.1 GB · 412 of 1 820 "
              "files", 12, color=MUTE), spacer(), T("11.4 MB/s", 12, color=MUTE)), gap=8),
              pad=(4, 20, 8, 20)),
        inset(row("Space", icon(20, 6, DARK), T("Laptop is running out of space: 2.0 GB "
                  "left to send, 1.2 GB free.", 13, "Medium", wrap=True), pad=14,
                  align="min", fills=[SURF], cornerRadius=12, strokes=[DARK],
                  strokeWeight=1.5)),
        section("Files", "Failed · 2"), inset(files),
        footer=row("Actions", button("Cancel job", primary=False, full=True),
                   button("Retry failed", full=True), gap=10, pad=[12, 16, 30, 16],
                   fills=[SURF], strokes=[LINE], strokeWeight=1, strokeAlign="inside"),
    )


# 7. delete on the other device
def delete_body():
    return [
        T("Delete 2 files on Dmytro's Laptop?", 20, "Bold"),
        T("IMG_0001.jpg, IMG_0002.jpg · 7.9 MB in Photos / Camera", 14, color=MUTE,
          wrap=True),
        row("Mode", icon(20, 6), col("Text", T("They go to the Trash on Dmytro's Laptop", 14,
                                                "Medium"),
                                      T("That's this folder's setting. They can be restored "
                                        "there.", 12, color=MUTE, wrap=True), gap=2),
            pad=14, align="min", fills=[BG], cornerRadius=12),
    ]


def d_delete():
    dlg = dialog_box(*delete_body(),
                     row("Actions", spacer(), link(button("Cancel", primary=False), "back"),
                         link(button("Move to Trash"), "back"), gap=10), w=500)
    return modal(d_split(), "Desktop / 24 Delete on other device", dlg)


def p_delete():
    return psheet("Phone / 26 Delete on other device", *delete_body(),
                  col("Actions", link(button("Move to Trash", full=True), "back"),
                      link(button("Cancel", primary=False, full=True), "back"), gap=8))


# ---------------------------------------------------------------- P2 screens
# 1. device details (Settings › Devices › a device)
DEVICE_FACTS = (("Platform", "macOS 15 · LocalDrop 0.4"), ("Device ID", "e012-6f4b-93cc"),
                ("Last seen", "Yesterday, 18:20 on Office-5G"), ("Known since", "2 Mar"))


def device_settings(phone=False):
    accept = card(
        list_row("Like the app default", "Now: automatically", lead=radio(False), pad_x=14),
        divider(48),
        list_row("Always automatically", None, lead=radio(False), pad_x=14),
        divider(48),
        list_row("Ask me every time", "Requests wait up to 10 min", lead=radio(True),
                 pad_x=14),
    )
    recv = card(
        default_row("Inbox for this device", "Like the app default"), divider(14),
        default_row("From its offline queue", "Like the app default"),
    )
    return accept, recv


def d_device():
    accept, recv = device_settings()
    facts = card(*[x for i, (k, v) in enumerate(DEVICE_FACTS) for x in
                   ([divider(14)] if i else []) + [srow(k, None, v)]])
    left = col("Left",
               row("Head", avatar(False, 48), col("Text", T("Work MacBook", 20, "Bold"),
                                                  T("Offline · seen yesterday", 13,
                                                    color=MUTE), gap=2),
                   T("Rename", 13, "Semi Bold"), gap=14),
               facts,
               link(row("Queue", T("Send queue", 14, "Medium"), spacer(),
                        T("Empty", 13, color=MUTE), T("Open  ›", 13, "Semi Bold"), gap=10,
                        pad=[12, 16, 12, 16], fills=[BG], cornerRadius=12), "d-offline"),
               gap=14)
    right = col("Right",
                T("ACCEPT FILES FROM IT", 11, "Semi Bold", MUTE, letterSpacing=0.6), accept,
                T("WHERE ITS FILES GO", 11, "Semi Bold", MUTE, letterSpacing=0.6), recv,
                gap=10)
    dlg = dialog_box(
        row("Columns", left, right, gap=28, align="min"),
        T("Shared folders it can open are set per folder (Settings › Shared folders).", 12,
          color=MUTE, wrap=True),
        row("Actions", link(T("Forget device", 13, "Semi Bold", MUTE), "back"), spacer(),
            link(button("Close", primary=False), "back"), link(button("Save"), "back"),
            gap=10),
        w=860)
    return modal(d_devices(), "Desktop / 25 Device details", dlg)


def p_device():
    accept, recv = device_settings(phone=True)
    facts = card(*[x for i, (k, v) in enumerate(DEVICE_FACTS) for x in
                   ([divider(16)] if i else []) + [list_row(k, v, lead=None, pad_x=16)]])
    return phone(
        "Phone / 27 Device details",
        app_bar("Work MacBook", "Offline · seen yesterday", back=True, actions=("Rename",)),
        section("Accept files from it"), inset(accept),
        section("Where its files go"), inset(recv),
        section("About"), inset(facts),
        inset(link(button("Forget device", primary=False, full=True), "back"),
              pad=(16, 16, 16, 16)),
    )


# 2. connect by address (when discovery can't see a device)
def ip_form():
    return [
        field("Address", "192.168.1.42", hint="IP address or name, e.g. desk-pc.local"),
        field("Port", "Auto", hint="Leave Auto unless the other device uses a fixed port "
              "(Settings › General).", trailing="47474"),
        check_opt("Remember this address", "Try it again whenever LocalDrop starts", True),
        row("Note", icon(20, 6), T("The device still has to prove it knows the group "
            "password. The address must be on a trusted network or an adapter you added "
            "(e.g. a VPN).", 12, color=MUTE, wrap=True), pad=12, align="min",
            cornerRadius=12, fills=[BG]),
    ]


def d_connect():
    dlg = dialog_box(
        T("Connect by address", 20, "Bold"),
        T("For networks where devices can't find each other (guest Wi-Fi, VPN, mDNS "
          "blocked).", 14, color=MUTE, wrap=True),
        *ip_form(),
        row("Actions", spacer(), link(button("Cancel", primary=False), "back"),
            link(button("Connect"), "d-remote"), gap=10),
        w=480)
    return modal(d_looking(), "Desktop / 26 Connect by address", dlg)


def p_connect():
    return psheet("Phone / 28 Connect by address",
                  T("Connect by address", 20, "Bold"),
                  col("Form", *ip_form(), gap=12),
                  col("Actions", link(button("Connect", full=True), "files-root"),
                      link(button("Cancel", primary=False, full=True), "back"), gap=8))


# 3. tray menu + system notifications (desktop)
def menu_item(label, sub=None, trailing=None, to=None, strong=False):
    kids = [col("Text", T(label, 13, "Semi Bold" if strong else "Medium"),
                *( [T(sub, 11, color=MUTE)] if sub else []), gap=1)]
    if trailing:
        kids.append(trailing if isinstance(trailing, dict) else T(trailing, 12, color=MUTE))
    r = row(f"Item / {label}", *kids, gap=10, pad=[7, 12, 7, 12], cornerRadius=6)
    return link(r, to) if to else r


def menu_sep():
    return F("Sep", R(10, 1, LINE, name="Line", sizingH="fill"), layout="horizontal",
             padding=[4, 0, 4, 0], sizingH="fill", sizingV="hug")


def toast(title, body, actions=(), lead=True):
    btns = [link(button(a, primary=i == len(actions) - 1, small=True), to)
            for i, (a, to) in enumerate(actions)]
    return col(f"Toast / {title}",
               row("Head", icon(20, 6, DARK), T("LocalDrop", 12, "Medium", MUTE), spacer(),
                   T("now", 11, color=FAINT), gap=8),
               T(title, 14, "Semi Bold", wrap=True), T(body, 13, color=MUTE, wrap=True),
               *( [row("Actions", spacer(), *btns, gap=8)] if btns else []),
               gap=6, pad=14, sizingH="fixed", w=360, fills=[SURF], cornerRadius=12,
               effects=[{"type": "dropShadow", "color": "#00000026", "offset": [0, 8],
                         "radius": 24}])


def d_tray():
    menu = col(
        "Tray menu",
        row("Head", icon(24, 7, DARK), col("Text", T("LocalDrop", 14, "Bold"),
                                           T("Group home · 3fa2-91c0", 11, color=MUTE),
                                           gap=0), pad=[6, 12, 6, 12]),
        menu_sep(),
        menu_item("Discovery", "Searching on Home", small_toggle(True), to="d-disc-off"),
        menu_sep(),
        T("ONLINE", 10, "Semi Bold", MUTE, letterSpacing=0.6, name="Online label"),
        menu_item("Dmytro's Laptop", "Sending Holiday 2024 · 34%", to="d-remote"),
        menu_item("Desk PC", "Receiving Backup.zip · 72%", to="d-transfers"),
        menu_sep(),
        menu_item("Open LocalDrop", strong=True, to="d-remote"),
        menu_item("Send files…", to="d-send"),
        menu_item("Pause all transfers", to="d-transfers"),
        menu_item("Settings", to="d-general"),
        menu_sep(),
        menu_item("Quit", "Queued files wait until you start it again"),
        gap=0, pad=[6, 6, 6, 6], sizingH="fixed", w=280, fills=[SURF], cornerRadius=10,
        strokes=[LINE], strokeWeight=1,
        effects=[{"type": "dropShadow", "color": "#00000026", "offset": [0, 8], "radius": 24}])
    toasts = col("Notifications",
                 T("NOTIFICATIONS", 11, "Semi Bold", MUTE, letterSpacing=0.6),
                 toast("Work MacBook wants to send 11 items", "1.2 GB · Contracts, "
                       "scan_0042.pdf and 9 more", (("Decline", "back"),
                                                    ("Review", "d-incoming"))),
                 toast("Holiday 2024 sent to Dmytro's Laptop", "1 820 files · 3.1 GB",
                       (("Show in Transfers", "d-transfers"),)),
                 toast("Not searching on “Office-5G”", "LocalDrop only searches on networks "
                       "you trust.", (("Not now", "back"), ("Trust", "d-new-net"))),
                 toast("iPad is back", "Sending 3 queued items", (("Open", "d-offline"),)),
                 gap=10, sizingH="hug")
    taskbar = row("Taskbar", icon(28, 6, FAINT, "Start"), spacer(),
                  row("Tray", icon(18, 4, FAINT), icon(18, 4, FAINT),
                      F("LocalDrop tray icon", icon(18, 4, DARK), layout="horizontal",
                        padding=4, sizingH="hug", sizingV="hug", fills=[BLOCK],
                        cornerRadius=6),
                      T("9:41", 12, "Medium"), gap=10, sizingH="hug"),
                  pad=[8, 16, 8, 16], fills=[SURF], strokes=[LINE], strokeWeight=1)
    desk = row("Desktop", col("Captions",
                              T("Tray menu", 16, "Bold"),
                              T("Right-click the tray icon. Closing the window keeps LocalDrop "
                                "running here, so queued files can still go out.", 13,
                                color=MUTE, wrap=True),
                              T("Notifications", 16, "Bold"),
                              T("Incoming requests, finished jobs, untrusted networks and "
                                "devices coming back. Clicking opens the matching screen.", 13,
                                color=MUTE, wrap=True),
                              gap=8, sizingH="fixed", w=300),
               spacer(), menu,
               toasts, gap=40, pad=[32, 32, 16, 32], align="max", sizingV="fill")
    return F("Desktop / 27 Tray menu & notifications", desk, taskbar, w=DESK[0], h=DESK[1],
             layout="vertical", gap=0, fills=[BLOCK], clip=True, cornerRadius=12,
             strokes=[LINE], strokeWeight=1)


def p_notifs():
    def note(title, body, actions=(), ongoing=False, bar=None):
        btns = [link(T(a, 13, "Semi Bold"), to) for a, to in actions]
        return col(f"Note / {title}",
                   row("Head", icon(18, 5, DARK), T("LocalDrop" + (" · ongoing" if ongoing
                                                                     else " · now"), 11,
                                                    "Medium", MUTE), gap=6),
                   T(title, 14, "Semi Bold", wrap=True), T(body, 13, color=MUTE, wrap=True),
                   *( [progress(bar)] if bar is not None else []),
                   *( [row("Actions", *btns, gap=20)] if btns else []),
                   gap=6, pad=14, fills=[SURF], cornerRadius=16)
    return phone(
        "Phone / 29 Notifications",
        col("Shade head", T("9:41", 40, "Bold"), T("Thursday, 14 November", 14, color=MUTE),
            gap=2, pad=[12, 20, 16, 20]),
        inset(col("Notes",
                  note("Sending Holiday 2024 to Dmytro's Laptop", "412 of 1 820 files · 3 min "
                       "left", (("Pause", "transfers"), ("Open", "details")), ongoing=True,
                       bar=0.34),
                  note("Work MacBook wants to send 11 items", "1.2 GB · waits up to 10 min",
                       (("Decline", "back"), ("Review", "incoming"))),
                  note("Not searching on “Office-5G”", "Open LocalDrop to trust this network.",
                       (("Open", "new-net"),)),
                  note("Received 3 files from Desk PC", "Saved to Download/LocalDrop",
                       (("Show", "transfers"),)),
                  gap=10), pad=(0, 12, 0, 12)),
        inset(T("The ongoing one is the Android foreground service that keeps transfers "
                "running; it goes away when nothing is queued. On iOS transfers run while the "
                "app is open.", 12, color=MUTE, wrap=True), pad=(14, 20, 0, 20)),
    )


# 4. mobile permissions after joining
def perm_row(title, sub, state, need):
    ctl = (chip("Allowed", True) if state == "ok" else
           button("Allow", small=True, primary=need == "Needed"))
    return row(f"Perm / {title}", icon(32, 9),
               col("Text", row("Title", T(title, 15, "Medium"), chip(need), gap=6,
                               sizingH="hug"),
                   T(sub, 12, color=MUTE, wrap=True), gap=3),
               ctl, pad=[12, 14, 12, 14], align="center")


def p_perms():
    rows = card(
        perm_row("Nearby devices", "Find your devices on this Wi-Fi. iOS: “Local Network”.",
                 "ok", "Needed"),
        divider(60),
        perm_row("Notifications", "Incoming files, finished jobs, untrusted networks.",
                 "ask", "Needed"),
        divider(60),
        perm_row("Run in background", "Lets queued files go out when a device appears. "
                 "Turns off battery optimisation for LocalDrop.", "ask", "Advised"),
        divider(60),
        perm_row("Wi-Fi name", "Tells your networks apart by name. Uses location; the name "
                 "never leaves the phone.", "ask", "Optional"),
    )
    return phone(
        "Phone / 30 Permissions",
        col("Hero", T("A few permissions", 24, "Bold"),
            T("LocalDrop asks only for what it uses. You can change these later in Settings.",
              14, color=MUTE, wrap=True), gap=6, pad=[24, 24, 16, 24]),
        inset(rows),
        inset(row("Files", icon(20, 6), T("No access to all your files: you pick each folder "
                  "you share, and files you receive go to Download/LocalDrop.", 12,
                  color=MUTE, wrap=True), pad=12, align="min", cornerRadius=12,
                  strokes=[LINE], strokeWeight=1), pad=(14, 16, 0, 16)),
        footer=col("Footer", link(button("Continue", full=True), "looking"),
                   link(T("Skip for now", 13, "Semi Bold", MUTE, name="Skip"), "looking"),
                   gap=12, crossAlign="center", pad=[12, 24, 30, 24]),
    )


# 5. desktop: inside a folder of the other PC (remote-only mode)
def d_remote_folder():
    w = (90, 110, 120)
    tbl = card(
        frow("Name", ("Size", "Type", "Modified"), w, header=True),
        divider(0),
        frow("2024", ("1 204 items", "Folder", "Today"), w, checked=False, trailing="›"),
        divider(48),
        frow("2023", ("980 items", "Folder", "3 Jan"), w, checked=False, trailing="›"),
        divider(48),
        frow("IMG_0001.jpg", ("4.1 MB", "JPEG image", "12 Mar"), w, checked=True),
        divider(48),
        frow("IMG_0002.jpg", ("3.8 MB", "JPEG image", "12 Mar"), w, checked=True),
        divider(48),
        frow("VID_0003.mp4", ("812 MB", "MP4 video", "14 Mar"), w, checked=False),
        divider(48),
        frow("notes.txt", ("2 KB", "Text", "15 Mar"), w, checked=False),
    )
    toolbar = row("Toolbar", side_toggle(True), link(nav_btns(), "d-remote"),
                  link(crumbs("Dmytro's Laptop", "Photos", "Camera"), "d-remote"), spacer(),
                  disc_pill(), search(110), seg("Remote", "Split", active=0), gap=10)
    access = row("Access", T("Your access in Photos:", 12, color=MUTE),
                 perm_chips("Browse", "Download", "Upload", "Delete → Trash"), spacer(),
                 T("Sort: Name ▾", 12, "Medium", MUTE), gap=8)
    actions = row("Actions", T("2 selected · 7.9 MB", 13, "Semi Bold"), spacer(),
                  button("Rename", primary=False, small=True),
                  link(button("Delete", primary=False, small=True), "d-delete"),
                  button("Download to…", primary=False, small=True),
                  link(button("Download", small=True), "d-transfers"), gap=8)
    hint = link(row("Drop hint", icon(20, 6), T("Drop files here to upload into Photos / "
                    "Camera. Downloads go to your Downloads folder unless you pick another.",
                    13, color=MUTE, wrap=True), pad=14, cornerRadius=12, strokes=[LINE],
                    strokeWeight=1), "d-drop")
    return window("Desktop / 28 Browse — remote folder", "laptop", main_area(
        toolbar, access, tbl, actions, hint, fill_v(), transfer_bar(), gap=14))


# 6. phone: left tab inside a folder, copy to the right tab
def p_local_folder():
    rows = card(
        p_row("IMG_2210.jpg", "3.6 MB · today", checked=True), divider(100),
        p_row("IMG_2211.jpg", "3.9 MB · today", checked=True), divider(100),
        p_row("VID_2212.mp4", "640 MB · today", checked=True), divider(100),
        p_row("IMG_2208.jpg", "4.0 MB · yesterday", checked=False), divider(100),
        p_row("IMG_2207.jpg", "3.7 MB · yesterday", checked=False),
    )
    target = link(row("Target", T("Copies to", 12, color=MUTE),
                      T("Dmytro's Laptop › Photos / Camera", 13, "Semi Bold"), spacer(),
                      T("Change", 12, "Semi Bold", MUTE), gap=6, pad=[8, 16, 8, 16],
                      fills=[SURF]), "files-remote")
    return phone(
        "Phone / 31 Files — left tab, in a folder",
        panel_tabs(0),
        path_bar("This phone", "DCIM", "Camera", up="files-local"),
        inset(rows, pad=(0, 12, 0, 12)),
        footer=col("Bottom", target,
                   row("Selection", T("3 selected · 648 MB", 13, "Semi Bold"), spacer(),
                       T("Select all", 13, "Medium", MUTE), pad=[8, 16, 8, 16], fills=[BG]),
                   cmd_bar(("Copy → right", True, "transfers"), ("Move → right", False),
                           ("Send to…", False, "send"), ("Delete", False), ("More", False)),
                   gap=0),
        nav="Files",
    )


# ---------------------------------------------------------------- settings (phone)
def p_group():
    return phone(
        "Phone / 14 Group",
        app_bar("Group", back=True),
        inset(col("Fingerprint", T("home", 15, "Medium", MUTE),
                  T("3fa2-91c0", 30, "Bold", letterSpacing=2),
                  T("Must match on every device in the group", 12, color=MUTE),
                  gap=6, pad=24, crossAlign="center", fills=[SURF], cornerRadius=16),
              pad=(8, 16, 0, 16)),
        section("Password"),
        inset(card(link(list_row("Change password", "Devices that don't get the new one "
                                 "drop out", lead=None, trailing="›", pad_x=16),
                        "change-pass"))),
        section("This device"),
        inset(card(list_row("This device's name", "Pixel 8", lead=None, trailing="›",
                            pad_x=16),
                   divider(16),
                   list_row("Device ID", "7d20-4be1-c38a", lead=None, pad_x=16),
                   divider(16),
                   link(list_row("Receive folder", "Download/LocalDrop", lead=None,
                                 trailing="›", pad_x=16), "recv"),
                   divider(16),
                   list_row("From offline queues", "Same as receive folder", lead=None,
                            trailing="›", pad_x=16))),
        inset(link(button("Leave group", primary=False, full=True), "leave"),
              pad=(20, 16, 0, 16)),
        inset(T("Leaving forgets the group key on this phone. You can join again with the "
                "same name and password.", 12, color=MUTE, wrap=True), pad=(8, 20, 0, 20)),
        nav="Settings",
    )


def p_known():
    def dev(name, sub, online, trailing=None):
        return list_row(name, sub, lead=F("Avatar", R(36, 36, BLOCK, 18), status_dot(online),
                                          layout="horizontal", gap=-10, crossAlign="max",
                                          sizingH="hug", sizingV="hug"),
                        trailing=trailing or "›", pad_x=16)
    dev = (lambda f: lambda *a: link(f(*a), "device"))(dev)
    return phone(
        "Phone / 15 Known devices",
        app_bar("Known devices", "4 devices in group home", back=True),
        inset(card(
            dev("Dmytro's Laptop", "Windows · Online · accepts automatically", True),
            divider(68),
            dev("Desk PC", "Linux · Online · accepts automatically", True),
            divider(68),
            dev("iPad", "iPadOS · 2 h ago · 3 queued", False, chip("3")),
            divider(68),
            dev("Work MacBook", "macOS · yesterday · asks before receiving", False),
        ), pad=(8, 16, 0, 16)),
        inset(row("Hint", icon(20, 6), T("Tap a device to rename it, set whether its files "
                  "need your OK, or forget it. Forgetting doesn't lock it out: change the "
                  "group password for that.", 13, color=MUTE, wrap=True), pad=14,
                  cornerRadius=12, strokes=[LINE], strokeWeight=1), pad=(16, 16, 0, 16)),
        nav="Settings",
    )


# ---------------------------------------------------------------- backups
BACKUP_SETS = [
    ("Today, 14:02", "Photos", "from Dmytro's Laptop · 12 files", "86 MB"),
    ("Yesterday, 19:40", "Projects", "from Work MacBook · 4 files", "310 MB"),
    ("3 Mar, 08:15", "Photos", "from Desk PC · 2 files", "16 MB"),
]


def d_backups():
    """Settings › Backups: old versions kept by "Replace, keep a backup" (local only)."""
    filt = col(
        "Share filter",
        T("SHARED FOLDERS", 11, "Semi Bold", MUTE, letterSpacing=0.6),
        side_item("All folders", True, sub="3 sets · 412 MB"),
        side_item("Photos", sub="2 sets · 102 MB"),
        side_item("Projects", sub="1 set · 310 MB"),
        side_item("Archive", sub="No backups"),
        gap=6, sizingH="fixed", w=240,
    )
    w = (90, 80)

    def set_head(when, share, sub, size, open_=False):
        return row(f"Set / {when}", T("▾" if open_ else "›", 14, "Medium", MUTE),
                   icon(28, 8, DARK if open_ else BLOCK),
                   col("Text", T(f"{when} · {share}", 14, "Semi Bold"), T(sub, 12, color=MUTE),
                       gap=2),
                   T(size, 13, "Medium", MUTE),
                   link(button("Restore set", primary=False, small=True), "d-restore"),
                   T("Delete", 13, "Semi Bold", MUTE), gap=12, pad=[12, 16, 12, 16],
                   fills=[BG] if open_ else [])

    files = col("Files",
                frow("Name", ("Size", ""), w, header=True, pad_x=56),
                frow("2024/IMG_0144.jpg", ("4.2 MB", "Restore"), w, checked=True, pad_x=56),
                frow("2024/IMG_0145.jpg", ("3.9 MB", "Restore"), w, checked=False, pad_x=56),
                frow("2024/VID_0146.mp4", ("61 MB", "Restore"), w, checked=False, pad_x=56),
                row("More", T("+ 9 more files", 12, "Medium", MUTE), pad=[6, 56, 10, 56]),
                gap=0)
    sets = card(
        set_head(*BACKUP_SETS[0], open_=True), files, divider(0),
        set_head(*BACKUP_SETS[1]), divider(0),
        set_head(*BACKUP_SETS[2]),
    )
    main = col(
        "Backups",
        row("Head", col("Text", T("Backups of replaced files", 18, "Bold"),
                        T("Kept on this PC only. Other devices can't see or delete them.", 13,
                          color=MUTE, wrap=True), gap=2),
            link(button("Clear all backups", primary=False, small=True), "d-clear-backups"),
            gap=8),
        row("Retention", T("Keep backups:", 13, color=MUTE), select_box("Until cleared"),
            T("App default", 12, color=MUTE), spacer(),
            T("1 selected · 4.2 MB", 13, "Medium", MUTE),
            button("Restore selected", small=True), gap=8),
        sets,
        T("Restoring puts the old version back; the current file goes to the Trash. Stored in "
          "<folder>/.localdrop/backups.", 12, color=MUTE, wrap=True),
        gap=12, sizingV="hug",
    )
    return window("Desktop / 05 Settings — backups", "settings", main_area(
        T("Settings", 24, "Bold"),
        tabs(*SETTINGS_TABS, active=5, links=SETTINGS_LINKS),
        row("Split", filt, main, gap=32, align="min"),
    ))


def p_backups():
    def bset(when, share, sub, size, open_=False):
        head = row("Head", icon(32, 9, DARK if open_ else BLOCK),
                   col("Text", T(f"{share} · {when}", 15, "Medium"),
                       T(f"{sub} · {size}", 12, color=MUTE), gap=2),
                   T("▾" if open_ else "›", 16, "Medium", FAINT), pad=[12, 16, 12, 16])
        if not open_:
            return head
        f = lambda n, sz: row(f"File / {n}", T(n, 13, "Medium"), spacer(),  # noqa: E731
                              T(sz, 12, color=MUTE), T("Restore", 12, "Semi Bold"), gap=10,
                              pad=[6, 16, 6, 60])
        return col("Set open", head, f("IMG_0144.jpg", "4.2 MB"), f("IMG_0145.jpg", "3.9 MB"),
                   f("VID_0146.mp4", "61 MB"),
                   row("More", T("+ 9 more", 12, "Medium", MUTE), pad=[4, 16, 6, 60]),
                   row("Set actions", button("Delete set", primary=False, small=True),
                       link(button("Restore set", small=True), "restore"), gap=8,
                       mainAlign="max",
                       pad=[6, 16, 12, 16]),
                   gap=0, fills=[BG])
    sets = card(bset(*BACKUP_SETS[0], open_=True), divider(64), bset(*BACKUP_SETS[1]),
                divider(64), bset(*BACKUP_SETS[2]))
    return phone(
        "Phone / 13 Backups",
        app_bar("Backups", "3 sets · 412 MB · on this phone only", back=True),
        inset(row("Filter", chip("All", True), chip("Camera"), chip("Documents"), chip("Music"),
                  gap=8), pad=(0, 20, 4, 20)),
        inset(row("Retention", col("Text", T("Keep backups", 15, "Medium"),
                                   T("App default · folders can override", 12, color=MUTE),
                                   gap=2),
                  select_box("Until cleared"), pad=14, fills=[SURF], cornerRadius=14),
              pad=(8, 16, 0, 16)),
        section("Backup sets"), inset(sets),
        footer=col("Footer", link(button("Clear all backups", primary=False, full=True),
                                  "clear-backups"), gap=0,
                   pad=[12, 16, 12, 16], fills=[SURF], strokes=[LINE], strokeWeight=1,
                   strokeAlign="inside"),
        nav="Settings",
    )


# ---------------------------------------------------------------- P3 screens
# 1. backups: confirm restore / clear all
RESTORE_FILES = (("2024/IMG_0144.jpg", "4.2 MB"), ("2024/IMG_0145.jpg", "3.9 MB"),
                 ("2024/VID_0146.mp4", "61 MB"))


def restore_body():
    files = card(*[x for i, (n, sz) in enumerate(RESTORE_FILES) for x in
                   ([divider(14)] if i else []) +
                   [row(f"File / {n}", T(n, 13, "Medium"), spacer(), T(sz, 12, color=MUTE),
                        pad=[9, 14, 9, 14])]],
                 row("More", T("+ 9 more files", 12, "Medium", MUTE), pad=[4, 14, 10, 14]))
    return [
        T("Restore 12 files to Photos?", 20, "Bold"),
        T("Backup set from today, 14:02 (replaced by Dmytro's Laptop) · 86 MB", 13,
          color=MUTE, wrap=True),
        files,
        row("Mode", icon(20, 6), col("Text", T("The current versions go to the Trash", 14,
                                                "Medium"),
                                      T("So restoring can be undone too. Files that were "
                                        "deleted since come back as they were.", 12,
                                        color=MUTE, wrap=True), gap=2),
            pad=14, align="min", fills=[BG], cornerRadius=12),
        check_opt("Keep this backup set", "Otherwise it's removed after restoring", True),
    ]


def clear_body():
    return [
        T("Delete all backups?", 20, "Bold"),
        T("3 sets · 412 MB in Photos and Projects. They are deleted permanently, not moved to "
          "the Trash. This can't be undone.", 14, color=MUTE, wrap=True),
        col("Sets", *[info_line(f"{w} · {s} · {sz}") for w, s, _, sz in BACKUP_SETS], gap=8),
        T("Current files in your shared folders aren't touched.", 12, color=MUTE, wrap=True),
    ]


def d_restore():
    dlg = dialog_box(*restore_body(),
                     row("Actions", spacer(), link(button("Cancel", primary=False), "back"),
                         link(button("Restore 12 files"), "back"), gap=10), w=520)
    return modal(d_backups(), "Desktop / 29 Restore backup", dlg)


def d_clear_backups():
    dlg = dialog_box(*clear_body(),
                     row("Actions", spacer(), link(button("Cancel", primary=False), "back"),
                         link(button("Delete 412 MB"), "back"), gap=10), w=480)
    return modal(d_backups(), "Desktop / 30 Clear all backups", dlg)


def p_restore():
    return psheet("Phone / 32 Restore backup", *restore_body(),
                  col("Actions", link(button("Restore 12 files", full=True), "back"),
                      link(button("Cancel", primary=False, full=True), "back"), gap=8))


def p_clear_backups():
    return psheet("Phone / 33 Clear all backups", *clear_body(),
                  col("Actions", link(button("Delete 412 MB", full=True), "back"),
                      link(button("Cancel", primary=False, full=True), "back"), gap=8))


# 2. queued files changed before a deferred job starts (SourceChanged)
def changed_body(phone=False):
    f = lambda n, why: row(f"File / {n}", icon(24, 6), col(  # noqa: E731
        "Text", T(n, 13, "Medium"), T(why, 12, color=MUTE), gap=1), pad=[8, 14, 8, 14])
    return [
        T("3 files changed since you queued them", 20, "Bold", wrap=True),
        T("“Photos for mom” → iPad, queued 2 h ago. iPad is online now.", 13, color=MUTE,
          wrap=True),
        card(f("IMG_0204.jpg", "Edited · 4.1 → 4.6 MB"), divider(52),
             f("IMG_0207.jpg", "Edited · 3.2 → 3.3 MB"), divider(52),
             f("IMG_0211.jpg", "Missing: deleted or moved")),
        col("Options",
            option("Send the current versions", "Changed files are sent as they are now; "
                   "the missing one is skipped.", selected=True),
            option("Skip changed files", "Send only the 45 files that didn't change."),
            gap=8),
    ]


def d_changed():
    dlg = dialog_box(*changed_body(),
                     row("Actions", link(T("Cancel job", 13, "Semi Bold", MUTE), "back"),
                         spacer(), link(button("Later", primary=False), "back"),
                         link(button("Continue"), "d-transfers"), gap=10), w=520)
    return modal(d_transfers(), "Desktop / 32 Queued files changed", dlg)


def p_changed():
    return psheet("Phone / 34 Queued files changed", *changed_body(True),
                  col("Actions", link(button("Continue", full=True), "transfers"),
                      link(button("Cancel job", primary=False, full=True), "back"), gap=8))


# 3. search on the other device
SEARCH_HITS = (("invoice-2024-03.pdf", "Projects / Clients / Acme", "184 KB", "12 Mar"),
               ("invoice-2024-02.pdf", "Projects / Clients / Acme", "176 KB", "9 Feb"),
               ("Invoices", "Projects / Finance", "38 items", "Yesterday"),
               ("invoice_template.docx", "Projects / Templates", "42 KB", "2 Jan"))


def d_search():
    w = (220, 80, 90)
    tbl = card(frow("Name", ("Folder", "Size", "Modified"), w, header=True), divider(0),
               *[x for i, (n, path, sz, m) in enumerate(SEARCH_HITS) for x in
                 ([divider(48)] if i else []) +
                 [frow(n, (path, sz, m), w, checked=i == 0)]])
    field_ = row("Search", icon(14, 4, DARK), T("invoice", 13, "Medium"), spacer(),
                 link(T("×", 14, "Medium", MUTE, name="Clear"), "d-remote"), gap=8,
                 pad=[7, 10, 7, 10], sizingH="fixed", w=240, fills=[SURF], strokes=[DARK],
                 strokeWeight=1.5, cornerRadius=8)
    toolbar = row("Toolbar", side_toggle(True), nav_btns(), crumbs("Dmytro's Laptop"), spacer(),
                  disc_pill(), field_, seg("Remote", "Split", active=0), gap=10)
    head = row("Head", col("Text", T("4 results for “invoice”", 20, "Bold"),
                           T("In folders shared with you · names only", 13, color=MUTE),
                           gap=2), spacer(),
               row("Scope", chip("All folders", True), chip("Photos"), chip("Projects"),
                   chip("Music"), gap=6, sizingH="hug"), gap=12)
    actions = row("Actions", T("1 selected · 184 KB", 13, "Semi Bold"), spacer(),
                  button("Show in folder", primary=False, small=True),
                  link(button("Download", small=True), "d-transfers"), gap=8)
    note = T("The search runs on Dmytro's Laptop. Folders it doesn't share with you are never "
             "searched.", 12, color=MUTE, wrap=True)
    return window("Desktop / 33 Search on other device", "laptop", main_area(
        toolbar, head, tbl, actions, note, fill_v(), transfer_bar(), gap=14))


def p_search():
    rows = card(*[x for i, (n, path, sz, m) in enumerate(SEARCH_HITS) for x in
                  ([divider(100)] if i else []) +
                  [p_row(n, f"{path} · {sz}", checked=i == 0,
                         trailing="›" if n == "Invoices" else None)]])
    bar = row("Search bar", link(T("‹", 22, "Medium", name="Up"), "back"),
              row("Field", icon(16, 4, DARK), T("invoice", 15, "Medium"), spacer(),
                  T("×", 16, "Medium", MUTE), gap=8, pad=[9, 12, 9, 12], fills=[SURF],
                  cornerRadius=10, strokes=[DARK], strokeWeight=1.5),
              gap=10, pad=[10, 16, 6, 16])
    return phone(
        "Phone / 35 Search on other device",
        panel_tabs(1), bar,
        inset(row("Scope", chip("All folders", True), chip("Photos"), chip("Projects"),
                  chip("Music"), gap=6), pad=(4, 16, 6, 16)),
        section("4 results on Dmytro's Laptop"),
        inset(rows, pad=(0, 12, 0, 12)),
        footer=cmd_bar(("Copy ← left", True, "transfers"), ("Show in folder", False),
                       ("Delete", False, "delete")),
        nav="Files",
    )


# 4. phone history (finished jobs)
def p_history():
    def h(name, sub, state):
        return row(f"Hist / {name}", icon(32, 8), col("Text", T(name, 14, "Medium"),
                                                       T(sub, 12, color=MUTE), gap=2),
                   chip(state, state == "Failed"), pad=[10, 14, 10, 14])
    today = card(h("Holiday 2024", "To Dmytro's Laptop · 1 820 files · 3.1 GB · 10:02", "Done"),
                 divider(60),
                 h("Notes.md", "From Dmytro's Laptop · 4 KB · 09:12", "Done"), divider(60),
                 h("Docs", "To Work MacBook · 240 of 242 · 08:55", "2 failed"))
    yday = card(h("Screenshots", "To Desk PC · 36 files · 18:40", "Done"), divider(60),
                h("old-build.iso", "To Work MacBook · stopped, 41%", "Cancelled"),
                divider(60),
                h("Contracts", "From Work MacBook · 11 items", "Declined"))
    return phone(
        "Phone / 36 Transfer history",
        app_bar("History", "Last 30 days · on this phone", back=True),
        inset(row("Filter", chip("All", True), chip("Sent"), chip("Received"), chip("Problems"),
                  gap=8), pad=(0, 20, 4, 20)),
        section("Today"), inset(link(today, "details")),
        section("Yesterday"), inset(yday),
        inset(T("Only the list is removed when you clear it; received files stay.", 12,
                color=MUTE, wrap=True), pad=(12, 20, 0, 20)),
        footer=col("Footer", button("Clear history", primary=False, full=True), gap=0,
                   pad=[12, 16, 12, 16], fills=[SURF], strokes=[LINE], strokeWeight=1,
                   strokeAlign="inside"),
        nav="Transfers",
    )


# 5. join errors
def err_field(label, value, error, trailing=None):
    box = row("Input", T(value, 15), spacer(),
              *( [T(trailing, 13, "Medium", MUTE)] if trailing else []),
              pad=[12, 14, 12, 14], fills=[SURF], strokes=[DARK], strokeWeight=2,
              cornerRadius=10)
    return col(f"Field / {label}", T(label, 13, "Medium", MUTE), box,
               row("Error", F("Bang", T("!", 11, "Bold", SURF), layout="horizontal",
                                mainAlign="center", crossAlign="center", w=16, h=16,
                                sizingH="fixed", sizingV="fixed", cornerRadius=8,
                                fills=[DARK]),
                   T(error, 12, "Medium", wrap=True), gap=6, align="min"), gap=6)


def join_banner(title, body, action, to=None):
    b = button(action, primary=False, small=True)
    return row("Banner", icon(20, 6, DARK), col("Text", T(title, 13, "Semi Bold"),
                                                T(body, 12, color=MUTE, wrap=True), gap=2),
               link(b, to) if to else b, pad=12, align="min", cornerRadius=12,
               strokes=[DARK], strokeWeight=1.5, fills=[SURF])


def d_join_error():
    win = d_join(err=True)
    win["name"] = "Desktop / 34 Join — errors"
    return win


def p_join_error():
    return phone(
        "Phone / 37 Join — errors",
        col("Hero", T("Join or create your group", 24, "Bold", wrap=True), gap=0,
            pad=[24, 24, 12, 24]),
        inset(join_banner("Local network access is off", "LocalDrop can't find your devices "
                          "without it. iOS: Settings › Privacy › Local Network.", "Open"),
              pad=(0, 20, 12, 20)),
        col("Form",
            field("Group name", "home"),
            err_field("Password", "sunny1", "Too short: 6 characters. Use at least 10, or "
                      "Generate a passphrase.", trailing="Generate"),
            field("This device's name", "Pixel 8"),
            trust_row(),
            gap=14, pad=[0, 24, 0, 24]),
        inset(T("Names and passwords are case-sensitive. If another device shows a different "
                "fingerprint, one of them is typed differently.", 12, color=MUTE, wrap=True),
              pad=(14, 24, 0, 24)),
        footer=col("Footer", button("Join group", full=True), gap=0, pad=[16, 24, 34, 24]),
    )


# 6. receive folder pickers
def recv_warnings():
    return [
        join_banner("This folder is synced by OneDrive", "OneDrive may start uploading files "
                    "while they're still arriving. Hidden partial files help, but not always.",
                    "Pick another"),
        row("Fat", icon(20, 6), T("Drive E: is FAT32: files of 4 GB or more can't be received "
                                  "there.", 12, color=MUTE, wrap=True), pad=12, align="min",
            cornerRadius=12, fills=[BG]),
    ]


def d_recv():
    dlg = dialog_box(
        T("Where received files go", 20, "Bold"),
        field("Inbox", "C:\\Users\\me\\OneDrive\\Downloads\\LocalDrop", trailing="Choose…",
              hint="128 GB free on C:"),
        recv_warnings()[0],
        row("Queue", col("Text", T("Files from offline queues", 14, "Medium"),
                         T("While you were away", 12, color=MUTE), gap=2),
            spacer(), select_box("Same as Inbox"), gap=10),
        row("Sub", checkbox(True), T("Put each job in its own subfolder when it has more than "
                                     "one item", 13, wrap=True), gap=10),
        T("Files sent into a shared folder go there instead. Devices can have their own Inbox "
          "(Settings › Devices).", 12, color=MUTE, wrap=True),
        row("Actions", spacer(), link(button("Cancel", primary=False), "back"),
            link(button("Save"), "back"), gap=10),
        w=560)
    return modal(d_general(), "Desktop / 35 Receive folder", dlg)


def p_recv():
    opts = col("Options",
               option("Download/LocalDrop", "Default. Shows up in Files and in the gallery.",
                      selected=True),
               option("Another folder…", "Pick it in the system folder picker; LocalDrop "
                      "keeps access only to that folder."),
               option("SD card / LocalDrop", "88 GB free · FAT32: files under 4 GB only"),
               gap=8)
    return phone(
        "Phone / 38 Receive folder",
        app_bar("Receive folder", "Where files sent to this phone go", back=True),
        inset(opts),
        section("From offline queues"),
        inset(card(default_row("While you were away", "Same folder"))),
        inset(row("Sub", checkbox(True), T("Put each job in its own subfolder", 13), gap=10),
              pad=(14, 20, 0, 20)),
        inset(T("Files sent into one of your shared folders go there instead.", 12,
                color=MUTE, wrap=True), pad=(12, 20, 0, 20)),
        nav="Settings",
    )


# ---------------------------------------------------------------- P4 screens
# 1. empty states
def empty_state(title, body, *actions, phone=False, name="Empty"):
    return col(name, icon(48 if not phone else 44, 14),
               T(title, 18 if not phone else 17, "Bold", textAlignHorizontal="center"),
               T(body, 13, color=MUTE, wrap=True, textAlignHorizontal="center"),
               *( [row("Actions", *actions, gap=10, sizingH="fill" if phone else "hug",
                       mainAlign="center")] if actions else []),
               gap=12, pad=28 if not phone else 20, crossAlign="center", fills=[SURF],
               cornerRadius=16)


def centered(box, w=480):
    return row("Center", F("Box", box, layout="vertical", sizingH="fixed", w=w,
                           sizingV="hug"), mainAlign="center")


def d_empty_transfers():
    toolbar = row("Toolbar", side_toggle(True), T("Transfers", 22, "Bold"),
                  T("Nothing in progress", 13, color=MUTE), spacer(), disc_pill(),
                  seg("All", "Outgoing", "Incoming", active=0, links={}), gap=12)
    box = empty_state(
        "Nothing is being sent", "Drag files onto a device in the sidebar, or right-click "
        "them in Explorer › Send with LocalDrop. Files for a device that's offline wait here "
        "until it's back.", link(button("Send files…"), "d-send"))
    hint = link(row("Hint", icon(20, 6), T("Your devices can't browse this PC yet: you don't "
                    "share any folders.", 13, color=MUTE, wrap=True),
                    T("Share a folder  ›", 13, "Semi Bold"), pad=[10, 14, 10, 14],
                    cornerRadius=10, strokes=[LINE], strokeWeight=1), "d-no-shares")
    return window("Desktop / 36 Transfers — empty", "transfers-empty", main_area(
        toolbar, fill_v("Top"), centered(box), centered(hint, 480),
        fill_v("Bottom"),
        T("Finished transfers stay in the list until you clear them; History keeps the last "
          "30 days.", 12, color=MUTE, textAlignHorizontal="center", wrap=True), gap=16))


def d_no_shares():
    box = empty_state(
        "You don't share any folders", "Devices in your group can't see any files on this "
        "PC. They can still send you files: those go to your Inbox (Downloads\\LocalDrop).",
        link(button("+  Share a folder"), "d-add-share"),
        link(button("Receive folder…", primary=False), "d-recv"))
    tips = col("Tips", info_line("A shared folder is browse + download only until you allow "
                                 "more."),
               info_line("You choose who sees it: all members or selected devices."), gap=8)
    return settings_window("Desktop / 37 Shared folders — empty", 1, fill_v("Top"),
                           centered(col("Wrap", box, tips, gap=16), 520), fill_v("Bottom"))


def d_remote_empty():
    w = (90, 270, 100)
    inbox_lead = F("Inbox icon", T("↓", 14, "Bold", SURF), layout="horizontal",
                   mainAlign="center", crossAlign="center", w=24, h=24, sizingH="fixed",
                   sizingV="fixed", cornerRadius=6, fills=[DARK])
    tbl = card(
        frow("Name", ("Size", "Your access", "Modified"), w, header=True), divider(0),
        link(frow("Inbox", ("—", perm_chips("Upload only"), "—"), w, checked=False,
                  lead=inbox_lead, sub="Files you send land in Downloads/LocalDrop",
                  trailing="›"), "d-send"))
    head = row("Device head", avatar(True), col("Text", T("Desk PC", 22, "Bold"),
               T("Linux · Online · shares no folders with you", 13, color=MUTE), gap=2),
               link(button("Send files…", primary=False, small=True), "d-send"), gap=14)
    toolbar = row("Toolbar", side_toggle(True), nav_btns(), crumbs("Desk PC"), spacer(),
                  disc_pill(), search(), seg("Remote", "Split", active=0), gap=12)
    box = empty_state("Desk PC doesn't share any folders with this PC",
                      "You can still send files to its Inbox. To browse its files, add a "
                      "shared folder on Desk PC (Settings › Shared folders) for all members "
                      "or for this PC.")
    return window("Desktop / 38 Browse — device shares nothing", "desk", main_area(
        toolbar, head, tbl, fill_v("Top"), centered(box, 520), fill_v("Bottom"),
        transfer_bar(), gap=16))


def p_empty_transfers():
    return phone(
        "Phone / 39 Transfers — empty",
        app_bar("Transfers", "Nothing in progress", actions=("Filter",)),
        inset(row("Tabs", chip("All", True), chip("Outgoing"), chip("Incoming"), spacer(),
                  link(T("History  ›", 13, "Semi Bold", name="History"), "history"), gap=8),
              pad=(4, 20, 4, 20)),
        inset(empty_state("Nothing is being sent", "Share files from any app and pick "
                          "LocalDrop, or select them in Files. Files for a device that's "
                          "offline wait here.",
                          link(button("Choose files", full=True), "files-local"),
                          phone=True), pad=(40, 16, 0, 16)),
        nav="Transfers")


def p_empty_folders():
    return phone(
        "Phone / 40 Shared folders — empty",
        app_bar("Shared folders", "What your group can see on this phone"),
        inset(empty_state("You don't share any folders", "Your devices can't see files on "
                          "this phone. They can still send you files: those go to the Inbox.",
                          link(button("+  Share a folder", full=True), "add-share"),
                          phone=True), pad=(24, 16, 0, 16)),
        section("Receiving"),
        inset(card(link(list_row("Inbox", "Download/LocalDrop · files sent to you",
                                 trailing="›", pad_x=16), "recv"))),
        nav="Folders")


def p_files_root_empty():
    return phone(
        "Phone / 41 Files — device shares nothing",
        panel_tabs(1, peer="Desk PC"),
        path_bar("Desk PC", actions=()),
        inset(row("Device", avatar(True, 36), col("Text", T("Desk PC", 15, "Semi Bold"),
                  T("Linux · Online · shares no folders", 12, color=MUTE), gap=2),
                  link(T("Switch", 13, "Semi Bold", INK, name="Switch device"), "devices"),
                  pad=14, fills=[SURF], cornerRadius=14), pad=(4, 12, 0, 12)),
        section("Send to this device"),
        inset(card(link(p_row("Inbox", "Upload only · lands in Downloads/LocalDrop",
                              lead=glyph_icon("↓"), trailing="›"), "send")),
              pad=(0, 12, 0, 12)),
        inset(empty_state("Nothing shared with you", "To browse Desk PC, add a shared folder "
                          "on it for all members or for this phone.", phone=True),
              pad=(16, 12, 0, 12)),
        nav="Files")


# 2. job action menus
def menu_box(*kids, w=260):
    return col("Menu", *kids, gap=0, pad=[6, 6, 6, 6], sizingH="fixed", w=w, fills=[SURF],
               cornerRadius=10, strokes=[LINE], strokeWeight=1,
               effects=[{"type": "dropShadow", "color": "#00000026", "offset": [0, 8],
                         "radius": 24}])


def danger_item(label, sub=None, to=None):
    """Destructive menu entry (red in the styled screens)."""
    r = menu_item(label, sub, to=to)
    r["name"] = f"Item / Danger {label}"
    return r


def d_job_menu():
    menu = menu_box(
        col("Menu head", T("contract.pdf", 13, "Semi Bold"),
            T("To Dmytro's Laptop · next in queue", 11, color=MUTE), gap=1, pad=[6, 12, 6, 12]),
        menu_sep(),
        menu_item("Pause", "Keeps its place in the queue"),
        menu_item("Move to top", "Sent right after the current file"),
        menu_item("Show details", to="d-details"),
        menu_item("Show in folder", "Where it's sent from"),
        menu_sep(),
        danger_item("Remove from queue", to="back"))
    win = d_transfers("Desktop / 39 Job actions")
    pop = F("Popover", menu, layout="vertical", sizingH="hug", sizingV="hug", absolute=True)
    pop["xy"] = (500, 326)
    win["kids"].append(pop)
    return win


def sheet_item(label, sub=None, to=None, danger=False):
    r = row(f"Item / {'Danger ' if danger else ''}{label}", icon(24, 7),
            col("Text", T(label, 15, "Medium"), *( [T(sub, 12, color=MUTE)] if sub else []),
                gap=1), pad=[10, 4, 10, 4])
    return link(r, to) if to else r


def p_job_menu():
    return psheet(
        "Phone / 42 Job actions",
        row("Head", icon(36, 10), col("Text", T("Backup.zip", 17, "Bold"),
            T("From Desk PC · 72% · 13.0 of 18.0 GB", 13, color=MUTE), gap=2)),
        col("Items",
            sheet_item("Pause", "Desk PC sends other files meanwhile"),
            sheet_item("Show details", to="details"),
            sheet_item("Open destination folder", "Folders/Backups"),
            sheet_item("Cancel transfer", "The partial file is deleted", to="back",
                       danger=True),
            gap=0),
        link(button("Close", primary=False, full=True), "back"))


# 3. Mirror confirm ("Replace the whole folder")
def mirror_rows(where):
    def r(n, text, sub, strong=False):
        return row(f"Set / {text}", F("Count", T(n, 15, "Bold", SURF if strong else INK),
                                      layout="horizontal", mainAlign="center",
                                      crossAlign="center", w=44, h=28, sizingH="fixed",
                                      sizingV="fixed", cornerRadius=8,
                                      fills=[DARK if strong else BLOCK]),
                   col("Text", T(text, 14, "Semi Bold" if strong else "Medium"),
                       T(sub, 12, color=MUTE, wrap=True), gap=2), gap=12, align="min")
    return col("Summary",
               r("230", "new files added", "Not in Projects yet."),
               r("12", "files replaced", f"Old versions go to the Trash {where}."),
               r("37", "files removed", f"They aren't in Docs. They go to the Trash {where}, "
                 "where you can restore them.", strong=True),
               gap=14)


def removed_list():
    return col("Files", *[row(f"Conflict / {n}", icon(20, 5), T(n, 13, "Medium"), spacer(),
                              T(s, 12, color=MUTE), gap=10, pad=[6, 12, 6, 12])
                          for n, s in (("old-plan.docx", "60 KB · 2023"),
                                       ("drafts/brief-v1.md", "4 KB · 2023"),
                                       ("archive/2022/", "18 files"))],
               row("More", T("+ 34 more", 12, "Medium", MUTE), spacer(),
                   T("Show all 37", 12, "Semi Bold"), pad=[6, 12, 6, 12]),
               gap=0, fills=[BG], cornerRadius=10)


def d_mirror():
    dlg = dialog_box(
        T("Make Projects an exact copy of Docs?", 20, "Bold"),
        T("Docs from Work MacBook → Projects on this PC. Files that aren't in Docs are "
          "removed from Projects.", 14, color=MUTE, wrap=True),
        mirror_rows("on this PC"),
        removed_list(),
        T("If this folder deleted permanently, you would also have to tick “I understand "
          "these files can't be restored”.", 12, color=MUTE, wrap=True),
        row("Actions", link(button("Back", primary=False), "back"), spacer(),
            link(button("Remove 37 files and replace"), "d-transfers"), gap=10),
        w=560)
    return modal(d_transfers("x"), "Desktop / 40 Replace whole folder", dlg)


def p_mirror():
    return psheet(
        "Phone / 43 Replace whole folder",
        T("Make Projects an exact copy of Docs?", 20, "Bold", wrap=True),
        T("Uploading Docs to Work MacBook › Projects. Files that aren't in Docs are removed "
          "there.", 14, color=MUTE, wrap=True),
        mirror_rows("on Work MacBook"),
        removed_list(),
        col("Actions", link(button("Remove 37 files and replace", full=True), "transfers"),
            link(button("Back", primary=False, full=True), "back"), gap=8))


# 4. receiver runs out of space mid-transfer
def space_banner(title, body, *actions):
    return row("Space", icon(20, 6, DARK),
               col("Text", T(title, 14, "Semi Bold"), T(body, 13, color=MUTE, wrap=True),
                   *( [row("Actions", *actions, gap=8, sizingH="hug")] if actions else []),
                   gap=6),
               pad=14, align="min", cornerRadius=12, fills=[SURF], strokes=[DARK],
               strokeWeight=1.5)


def d_low_space():
    files = card(
        row("Head", T("Files", 14, "Semi Bold"), spacer(),
            row("Filter", chip("All", True), chip("Waiting · 50"), chip("Done · 35"), gap=6,
                sizingH="hug"), pad=[12, 16, 6, 16]),
        file_state("VID_0231.mp4", "2.1 GB", "Paused", "1.3 GB received and kept · continues "
                   "from there", strong=True),
        divider(56),
        file_state("VID_0232.mp4", "1.8 GB", "Waiting"), divider(56),
        file_state("VID_0233.mp4", "940 MB", "Waiting"), divider(56),
        file_state("VID_0230.mp4", "1.2 GB", "Done"))
    head = row("Header", link(T("‹", 26, "Medium", name="Back"), "back"), icon(40, 12),
               col("Text", T("Videos from Desk PC", 20, "Bold"),
                   T("86 files · 24.0 GB · into Downloads\\LocalDrop\\Videos", 13,
                     color=MUTE), gap=2),
               chip("Paused · low on space", True),
               link(button("Cancel job", primary=False, small=True), "back"), gap=12)
    stats = row("Stats", *[col(f"Stat {k}", T(k, 12, color=MUTE), T(v, 16, "Semi Bold"), gap=2,
                               sizingH="fill")
                           for k, v in (("Received", "9.8 of 24.0 GB"), ("Files", "35 of 86"),
                                        ("Still needed", "14.2 GB"), ("Free on C:", "0.9 GB"))],
                gap=16, pad=16, fills=[SURF], cornerRadius=12)
    banner = space_banner(
        "Paused: drive C: is almost full",
        "LocalDrop stopped before the disk filled up. Free up 13.3 GB and it continues on "
        "its own (checked every 30 s), or put the rest of this job into another folder.",
        button("Open folder", primary=False, small=True),
        button("Put the rest elsewhere…", primary=False, small=True))
    return window("Desktop / 41 Low space while receiving", "transfers", main_area(
        head, col("Progress", progress(0.41), gap=0), stats, banner, files,
        T("Desk PC shows “Paused · receiver low on space” and can cancel too.", 12,
          color=MUTE), gap=14))


def p_low_space():
    inc = card(job("Videos from Desk PC", "35 of 86 files · into Download/LocalDrop", 0.41,
                   state="Paused · low on space", meta="9.8 of 24.0 GB",
                   action=link(T("⋮", 18, "Medium", MUTE, name="More"), "job-menu")))
    return phone(
        "Phone / 44 Low space while receiving",
        app_bar("Transfers", "1 paused", actions=("Filter",)),
        inset(space_banner("Phone storage is almost full",
                           "Videos from Desk PC paused: 14.2 GB still to come, 0.9 GB free. "
                           "It continues by itself when there's space.",
                           button("Free up space", primary=False, small=True),
                           link(button("Other folder", primary=False, small=True), "recv")),
              pad=(4, 16, 0, 16)),
        section("Incoming"), inset(inc),
        inset(T("Files already received stay. Desk PC sees that the job is paused.", 12,
                color=MUTE, wrap=True), pad=(12, 20, 0, 20)),
        nav="Transfers")


SCREENS = {
    "d-join": (d_join, 99),
    "d-remote": (d_remote, 100),
    "d-split": (d_split, 101),
    "d-offline": (d_offline, 102),
    "d-settings": (d_settings, 103),
    "d-backups": (d_backups, 104),
    "d-transfers": (d_transfers, 105),
    "d-conflict": (d_conflict, 106),
    "d-general": (d_general, 107),
    "d-defaults": (d_defaults, 108),
    "d-devices": (d_devices, 109),
    "d-disc-off": (d_disc_off, 110),
    "d-net-menu": (d_net_menu, 111),
    "d-new-net": (d_new_net, 112),
    "d-networks": (d_networks, 113),
    "d-change-pass": (d_change_pass, 114),
    "d-leave": (d_leave, 115),
    "d-looking": (d_looking, 116),
    "d-incoming": (d_incoming, 117),
    "d-key": (d_key, 118),
    "d-drop": (d_drop, 119),
    "d-send": (d_send, 120),
    "d-add-share": (d_add_share, 121),
    "d-details": (d_details, 122),
    "d-delete": (d_delete, 123),
    "join": (p_join, 0),
    "files-remote": (p_files_remote, 1),
    "send": (p_send, 2),
    "transfers": (p_transfers, 3),
    "files-local": (p_files_local, 4),
    "conflict": (p_conflict, 5),
    "share": (p_share_settings, 6),
    "files-offline": (p_files_offline, 7),
    "files-root": (p_files_root, 8),
    "devices": (p_devices_sheet, 9),
    "folders": (p_folders, 10),
    "settings": (p_settings, 11),
    "backups": (p_backups, 12),
    "group": (p_group, 13),
    "known": (p_known, 14),
    "disc-off": (p_disc_off, 15),
    "new-net": (p_new_net, 16),
    "networks": (p_networks, 17),
    "change-pass": (p_change_pass, 18),
    "leave": (p_leave, 19),
    "looking": (p_looking, 20),
    "incoming": (p_incoming, 21),
    "key": (p_key, 22),
    "add-share": (p_add_share, 23),
    "details": (p_details, 24),
    "delete": (p_delete, 25),
    "d-device": (d_device, 124),
    "d-connect": (d_connect, 125),
    "d-tray": (d_tray, 126),
    "d-remote-folder": (d_remote_folder, 127),
    "device": (p_device, 26),
    "connect": (p_connect, 27),
    "notifs": (p_notifs, 28),
    "perms": (p_perms, 29),
    "local-folder": (p_local_folder, 30),
    "d-restore": (d_restore, 128),
    "d-clear-backups": (d_clear_backups, 129),
    "d-changed": (d_changed, 130),
    "d-search": (d_search, 131),
    "d-split-side": (lambda: d_split(side=True), 132),
    "d-join-error": (d_join_error, 133),
    "d-recv": (d_recv, 134),
    "restore": (p_restore, 31),
    "clear-backups": (p_clear_backups, 32),
    "changed": (p_changed, 33),
    "search": (p_search, 34),
    "history": (p_history, 35),
    "join-error": (p_join_error, 36),
    "recv": (p_recv, 37),
    "d-empty-transfers": (d_empty_transfers, 135),
    "d-no-shares": (d_no_shares, 136),
    "d-remote-empty": (d_remote_empty, 137),
    "d-job-menu": (d_job_menu, 138),
    "d-mirror": (d_mirror, 139),
    "d-low-space": (d_low_space, 140),
    "empty-transfers": (p_empty_transfers, 38),
    "empty-folders": (p_empty_folders, 39),
    "files-root-empty": (p_files_root_empty, 40),
    "job-menu": (p_job_menu, 41),
    "mirror": (p_mirror, 42),
    "low-space": (p_low_space, 43),
}


# ---------------------------------------------------------------- emit
def emit(node, parent, steps, xy=None):
    ref = f"n{len(steps)}"
    params = {"type": node["type"], "name": node["name"], "props": dict(node["props"]),
              "select": False}
    if node.get("link"):
        params["props"]["pluginData"] = {"link": node["link"]}
    if node.get("text") is not None:
        params["text"] = node["text"]
    if node.get("path"):
        params["path"] = node["path"]
    if node.get("w"):
        params["width"] = node["w"]
    if node.get("h"):
        params["height"] = node["h"]
    if parent:
        params["parent"] = {"$ref": parent}
    xy = xy or node.get("xy")
    if xy:
        params["x"], params["y"] = xy
    steps.append({"command": "node.create", "as": ref, "params": params})
    for k in node["kids"]:
        emit(k, ref, steps)
    return ref


def top_level_ids(name):
    """Ids of top-level layers on the current page with this name."""
    m = fc.call("inspect_document", {"depth": 1, "fields": ["id", "name", "type"]})
    doc = json.loads(m["result"]["content"][0]["text"])
    page = next(p for p in doc["pages"] if p["id"] == doc["currentPage"])
    return [k["id"] for k in page.get("children", []) if k.get("name") == name]


def build(name):
    fn, slot = SCREENS[name]
    if slot < 99:
        xy = (slot * (PHONE[0] + GAP_X), 0)
    else:
        xy = ((slot - 99) * (DESK[0] + GAP_X), PHONE[1] + 200)
    tree = fn()
    old = top_level_ids(tree["name"])
    if old:
        fc.call("run_command", {"command": "edit.delete", "params": {"ids": old}})
    steps = []
    emit(tree, None, steps, xy)
    m = fc.call("batch", {"commands": steps, "label": f"Wireframe: {name}"})
    if "error" in m or m.get("result", {}).get("isError"):
        print(name, "FAILED:", json.dumps(m)[:1500])
        return None
    first = json.loads(m["result"]["content"][0]["text"])[0]
    print(name, "ok, steps:", len(steps), "root id:", first.get("id"))
    return first.get("id")


# ---------------------------------------------------------------- prototype
# Event-driven states (no click leads there) get their own flow as the way in.
FLOWS = {"Desktop": "d-join", "Phone": "join",
         "Desktop · new network": "d-new-net", "Phone · new network": "new-net",
         "Desktop · incoming request": "d-incoming", "Phone · incoming request": "incoming",
         "Desktop · key changed": "d-key", "Phone · key changed": "key",
         "Desktop · tray & notifications": "d-tray", "Phone · notifications": "notifs",
         "Desktop · queued files changed": "d-changed", "Phone · queued files changed": "changed",
         "Desktop · join errors": "d-join-error", "Phone · join errors": "join-error",
         "Desktop · no transfers": "d-empty-transfers", "Phone · no transfers": "empty-transfers",
         "Desktop · nothing shared": "d-no-shares", "Phone · nothing shared": "empty-folders",
         "Desktop · low space (receiving)": "d-low-space",
         "Phone · low space (receiving)": "low-space"}


def _text(m):
    if "error" in m or m.get("result", {}).get("isError"):
        raise RuntimeError(json.dumps(m)[:800])
    return json.loads(m["result"]["content"][0]["text"])


def _links(node, out):
    if (node.get("pluginData") or {}).get("link"):
        out.append((node["id"], node["pluginData"]["link"]))
    for k in node.get("children") or []:
        _links(k, out)


def wire():
    """Re-link every screen: layers tagged with link() navigate to their target frame.

    Runs after every build, so a new or rebuilt screen is clickable right away.
    """
    names = {SCREENS[k][0]()["name"]: k for k in SCREENS}
    doc = _text(fc.call("inspect_document", {"depth": 1, "fields": ["id", "name", "flows"]}))
    page = next(p for p in doc["pages"] if p["id"] == doc["currentPage"])
    frames = {names[c["name"]]: c["id"] for c in page.get("children", [])
              if c.get("name") in names}
    links = []
    for fid in frames.values():
        node = _text(fc.call("get_node", {"id": fid, "depth": 60, "fields": ["pluginData"],
                                          "maxBytes": 4000000}))
        if node.get("truncated"):
            print("warning: truncated tree for", fid)
        _links(node, links)
    steps = [{"command": "proto.interaction.clear", "params": {"ids": [i for i, _ in links]}}]
    for fl in page.get("flows") or []:
        start = fl.get("start") or fl.get("startingPoint") or fl.get("id")
        if start:
            steps.append({"command": "proto.flow.remove", "params": {"start": start}})
    for name, key in FLOWS.items():
        if key in frames:
            steps.append({"command": "proto.flow.add",
                          "params": {"start": frames[key], "name": name}})
    missing = set()
    for nid, target in links:
        if target == "back":
            p = {"id": nid, "trigger": {"type": "onClick"}, "action": {"type": "back"}}
        elif target in frames:
            p = {"id": nid, "trigger": {"type": "onClick"},
                 "action": {"type": "navigate", "dest": frames[target]},
                 "transition": {"type": "dissolve"}, "durationMs": 150, "easing": "easeOut"}
        else:
            missing.add(target)
            continue
        steps.append({"command": "proto.interaction.add", "params": p})
    m = fc.call("batch", {"commands": steps, "label": "Wireframe: prototype links"})
    if "error" in m or m.get("result", {}).get("isError"):
        print("wire FAILED:", json.dumps(m)[:1500])
        return
    print(f"wired {len(links) - sum(1 for _, t in links if t in missing)} links on "
          f"{len(frames)} screens" + (f"; missing screens: {sorted(missing)}" if missing else ""))


if __name__ == "__main__":
    if sys.argv[1] == "list":
        print("\n".join(SCREENS))
    elif sys.argv[1] == "wire":
        wire()
    else:
        for s in (list(SCREENS) if sys.argv[1] == "all" else sys.argv[1:]):
            build(s)
        wire()
