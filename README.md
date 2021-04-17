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

See [doc](doc) directory for additional documentation.
