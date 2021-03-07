slow4: slow4.c
	gcc `pkg-config --cflags gtk4` -o slow4 slow4.c `pkg-config --libs gtk4`

slow3: slow3.c
	gcc `pkg-config --cflags gtk+-3.0` -o slow3 slow3.c `pkg-config --libs gtk+-3.0`
