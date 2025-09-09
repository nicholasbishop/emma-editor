# Architecture

## Packages

* `emma_app` (`app` directory) - Most of the code is here, the notably
  includes the application state (`struct AppState`), which includes
  buffers, the pane tree, etc.
* `emma_gtk_shell` (`gtk_shell` directory) - A fairly simple GTK
  application. This package initializes a GTK app and creates an
  instance of `AppState`. It handles drawing and sending keyboard events
  to the `AppState` for processing.

## Data flow

The GKT app owns the `AppState` instance, and can read from it directly
(e.g. for drawing). Keyboard events are converted to types defined in
`app` and passed to `AppState::handle_key_press`.

The GTK app also creates a message pipe. The writer end is passed to
`AppState` so that it can send back messages. These messages are used
for two purposes:
* To handle a GTK-specific event, such as closing a window.
* To process an event from a thread in the app crate. For example, when
  a process is launched in the app crate, a thread is created to read
  output from the process. This data can't be added directly to the
  associated buffer within the app's main thread, because the buffer
  data is not shared across threads. Instead, the reader thread writes
  it to the message pipe, and it's read by a handler in the main loop of
  the GTK process when data becomes available. This data then gets
  passed to `AppState::handle_action`.

