Build deps:

    dnf install gtksourceview3-devel

Build/test/run:

    ./cycle_client
    
TODOs:

* Implement next/prev view
* Implement close view
* Implement view layout history
* With splits, prevent view sizes from increasing as you type in them
* Implement buffers not tied to views
* Implement opening stuff
* Implement minibuf
* Implement info line in views
* Implement persistence
* Figure out the SSH stuff

## Syntax Highlighting

The sourceview widget has built-in syntax highlighting, but it's
extremely limited in what it can highlight. So instead we use the
syntect library which is much more powerful, but harder to integrate.

See `src/bin/client/highlight.rs`.
