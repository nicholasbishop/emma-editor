This branch works, but for some reason performance is very bad. For
example, open a file and hold down ctrl-F -- it moves very slowly.

Profiling shows that drawing is really slow. Stuff in libpixman at the
top of the chart. Decreasing window size makes it go faster.
