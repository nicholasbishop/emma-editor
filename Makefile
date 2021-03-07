slow: slow.c
	gcc `pkg-config --cflags gtk4` -o slow slow.c `pkg-config --libs gtk4`
