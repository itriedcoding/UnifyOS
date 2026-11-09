CC ?= cc
PKG_CONFIG ?= pkg-config
CFLAGS ?= -O2 -g -Wall -Wextra -Wno-unused-parameter
GTK_CFLAGS ?= $(shell $(PKG_CONFIG) --cflags gtk4)
GTK_LIBS ?= $(shell $(PKG_CONFIG) --libs gtk4)

APPS := $(patsubst %.c,%,$(filter-out libunify.c,$(wildcard unify-*.c)))

all: $(APPS)

$(APPS): %: %.c libunify.c libunify.h
	$(CC) $(CFLAGS) $(GTK_CFLAGS) -o $@ $< libunify.c $(GTK_LIBS)

install: all
	install -d "$(DESTDIR)/usr/bin"
	install -m755 $(APPS) "$(DESTDIR)/usr/bin/"
	install -d "$(DESTDIR)/usr/share/unify"
	install -m644 unify.css "$(DESTDIR)/usr/share/unify/unify.css"

clean:
	rm -f $(APPS)

.PHONY: all install clean
