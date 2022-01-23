# emma

Emma is a text editor.

It is currently just a side project and not what you might call "functional". :)

## Code layout

- [main.rs](src/main.rs): basic gtk4 setup
- [app.rs](src/app.rs): top-level application state
- [app/draw.rs](src/src/app.rs): draw the app using Cairo
- [app/event.rs](src/app/event.rs): event handling
- [key_sequence.rs](src/key_sequence.rs): groups individual key
  presses into sequences
- [key_map.rs](src/key_map.rs): maps key sequences to actions
- [buffer.rs](src/buffer.rs): buffers represent something being
  edited, e.g. a text file
- [pane_tree.rs](src/pane_tree.rs): tree of panes, where each pane
  shows a buffer
- [theme.rs](src/theme.rs): YAML format for themes (see also
  [emma.theme.yml](src/emma.theme.yml))

See [doc](doc) directory for additional documentation.
