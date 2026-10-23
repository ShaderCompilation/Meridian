# Meridian

A fast, keyboard-first, fully offline Kanban board for the desktop, written in Rust with [iced](https://iced.rs).

- Boards → sections → tasks, with descriptions, checklists, labels, priorities and due dates
- Everything works from the keyboard (vim keys and arrows), plus a fuzzy command palette (`Ctrl+K`)
- Mouse drag and drop for tasks and sections
- Unlimited undo/redo for every change
- Dark and light themes that follow the system
- Data is stored locally in SQLite: `~/.local/share/meridian/meridian.db`

## Build & run

```sh
cargo run --release
```

On Linux, building needs a C compiler (`gcc`) for the bundled SQLite.

Environment variables:

| Variable | Purpose |
|---|---|
| `MERIDIAN_DB` | Use a different database file |
| `ICED_BACKEND=tiny-skia` | Use the software renderer instead of the GPU |
| `WGPU_BACKEND=gl` | Use OpenGL instead of Vulkan |

## Keyboard

Press `?` in the app for the full cheat sheet. The essentials:

| Keys | Action |
|---|---|
| `h j k l` / arrows | Move between sections and tasks |
| `H J K L` / `Shift+arrows` | Move the selected task (or section) |
| `Enter` / `e` | Open task details (on a section header: rename) |
| `n` / `o`, `O` | New task below / above (Enter keeps adding) |
| `N` | New section |
| `x` / `Space` | Toggle done (moves the task to the “Done” section) |
| `dd` / `Delete` | Delete task or section |
| `p` (then `0–4`) | Priority |
| `t` | Labels |
| `D` | Due date (`today`, `tom`, `fri`, `+3d`, `2w`, `nov 3`…) |
| `m` | Move to section |
| `y` | Duplicate |
| `z` | Collapse / expand section |
| `/` | Filter: words, `#label`, `!high`, `@today`, `@overdue`, `@week`, `@done`… |
| `Ctrl+K` / `:` | Command palette |
| `Ctrl+P` | Go to any task |
| `b`, `Ctrl+1…9`, `[` `]` | Switch boards |
| `u` / `Ctrl+Z`, `U` / `Ctrl+Shift+Z` | Undo / redo |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+Shift+L` | Toggle light / dark |

In task details: `j/k` select checklist items, `Space` toggles, `J/K` reorders, `a` adds, `i` edits the description, `r` the title, `Tab` cycles fields, `Esc` closes.

## Development

```sh
cargo test
cargo clippy
```

`tools/shoot.sh <dir> "<script>"` drives the UI with a scripted sequence of keys and saves screenshots
(see `src/devtools.rs` for the script syntax), e.g.:

```sh
tools/shoot.sh /tmp/shots "j;l;Enter;wait:300;shot:detail;Esc;C-k;shot:palette;quit"
```

## Layout

```
src/
  main.rs       window setup, fonts
  app.rs        state, messages, update loop, keyboard handling, data operations
  model.rs      data types, in-memory store, ordering
  db.rs         SQLite schema, migrations, persistence
  palette.rs    command palette modes, fuzzy ranking, actions
  commands.rs   command registry (palette + cheat sheet)
  filter.rs     filter query language
  dates.rs      natural-language dates
  theme.rs      design tokens and widget styles
  ui/           views (board, cards, detail panel, palette, sidebar, help)
```
