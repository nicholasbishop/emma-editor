# Gtk

Emma uses gtk4-rs to put a window on the screen, draw its contents,
and receive input. Only one widget is used, a DrawingArea that covers
the whole window. Everything inside it is manually drawn using cairo
and pango rather than trying to manage a tree of Gtk widgets.

## Why Gtk

There are a number of Rust projects trying to figure out how best to
implement a GUI. Eventually using one of them might make more
sense. At the moment the most promising project is Druid, but they are
using Gtk in the backend very similar to how Emma is, so currently I
don't think there's much benefit to switching.

I think Qt would probably also work fine as a backend, but I'm not
aware of any Rust crates that wrap Qt as conveniently as the Gtk
wrappers.

I'm developing this on Linux, so Gtk works well. I'm not sure what
support on Windows or MacOS looks like these days.

The choice of Gtk4 over Gtk3 is somewhat arbitrary, but it's the
future so might as well use it.

## Why a single DrawingArea instead of normal widgets

Initially I tried using GtkSourceView for each pane. I gave up on that
when I realized the built-in syntax highlighting is extremely limited,
and none of the other features relative to GtkTextView seemed to
matter much.

Then I tried using GtkTextView. In Gtk3 I couldn't have a tree of them
because of a crash. That crash isn't a problem in Gtk4, but then I
started working on cursor drawing and couldn't find a good way to make
the cursor look like how I wanted. I figured that I might as well just
make it fully custom so I can make it work exactly how I want.

The other motivation for doing everything manually is to avoid the
heavy use of reference counting in Gtk. Emma by necessity has a lot of
state that's pretty global (e.g. buffers and panes often need to know
about each other), but I've pushed most of that state up to the App
struct, and then lower down I can use functions that take refs and mut
refs rather than dealing with a bunch of `Rc<Cell>` type things. All
in all I think this design feels a lot nicer than it did when I tried
to do things in a normal Gtkish way, especially in parts like
`PaneTree`.
