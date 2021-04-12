# emma

Emma is a text editor.

## Code layout

- main.rs: basic gtk4 setup
- app.rs: top-level application state
- app/draw.rs: draw the app using Cairo
- app/event.rs: event handling
- key_sequence.rs: groups individual key presses into sequences
- key_map.rs: maps key sequences to actions
- buffer.rs: buffers represent something being edited, e.g. a text file
- pane_tree.rs: tree of panes, where each pane shows a buffer
- theme.rs: YAML format for themes (see also `emma.theme.yml`)

## Dev

This code is in heavy development. For my own reference, here are some
recent useful branches:

- bishop-custom-textview-2
- bishop-main-backup-20210328

Some todos:
- Close pane
- Switch buffer via minibuf
- Close buffer
- Async highlighting
- SSH support
- Persistence
- Winner undo/redo
- Text undo/redo
- Search
- dabrev
- Compilation buffer
- Multi-window support
- Workspace switcher
- Shell
- Interactive search
- git-root-[rip]grep
- git-root-{fd|find}
