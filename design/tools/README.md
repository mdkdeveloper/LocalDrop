# Wireframe tools

Scripts that draw the LocalDrop wireframes into a running FigCraft app.

- `fc.py` — tiny MCP client: starts `figcraft-cli mcp --attach 47801` and calls one tool.
- `wf.py` — the wireframes, described as auto-layout trees; each screen is built as one
  undo step. Rebuilding a screen deletes the frame with the same name first.

```bash
# FigCraft must be running with the control port:
"/c/Program Files/ArtCraft/FigCraft/figcraft.exe" --control 47801 --automation-write-root "D:/Repos/github/LocalDrop/design"

python wf.py list                 # screen names
python wf.py d-join browse        # rebuild some screens
python wf.py all                  # rebuild everything
python fc.py call screenshot '{"ids":[123]}'   # render to out/shot.png
python fc.py call save_file '{}'
```

## Clickable prototype

The wireframes are a clickable prototype (flows **Desktop** → `d-join`, **Phone** → `join`,
plus flows for event-driven states; open with Present or Shift+Space in FigCraft).

- In a screen function, wrap a layer with `link(node, "<screen key>")` to navigate there
  on click, or `link(node, "back")` to go back. The target is stored in the layer's
  `pluginData`.
- `wf.py` re-links the whole page after every build (`wire()`), so the frame ids that
  change on rebuild never break links. `python wf.py wire` does only that step.

**Rule: every new screen is linked into the prototype in the same change.** Add it to
`SCREENS`, give it at least one way in (a `link()` from an existing screen) and a way out
(its own nav, a back link or buttons), then build. A state that no click leads to (it
is triggered by an event, e.g. joining a new network) gets its own entry in `FLOWS`. `wire` reports links that point to
screens that don't exist yet.

Renders go to `out/` (git-ignored). The design file itself is `../localdrop-wireframes.figcraft`.
