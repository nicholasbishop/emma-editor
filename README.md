Build deps:

    dnf install graphene-devel gtk4-devel

Build/test/run:

    ./cycle_client
    
TODOs:

* Completion for minibuf file operations
* Save buffer
* Change active view when clicking to focus a different pane
* Implement persistence
* Close view
* View layout history
* M-x style commands in minibuf
* Add line/col to info bar
* Something like dabbrev-expand (M-/)
* Figure out the SSH stuff
* Many TODOs in the code

## Syntax Highlighting

The sourceview widget has built-in syntax highlighting, but it's
extremely limited in what it can highlight. So instead we use the
syntect library which is much more powerful, but harder to integrate.

See `src/bin/client/highlight.rs`.

Reference:

Rust Sublime syntax: https://github.com/rust-lang/rust-enhanced/blob/master/RustEnhanced.sublime-syntax
Textmate docs: https://macromates.com/manual/en/scope_selectors

## Actions

All actions are defined in the `Action` enum in
`src/bin/client/key_map.rs`. That file also has the default
keymap. The actual implementations for actions are currently in
`src/bin/client/main.rs`.
