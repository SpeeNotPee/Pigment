# Pigment install rules.
#
# Usage:
#   make build                 # compile release binaries
#   make install               # install into PREFIX (default /usr/local)
#   make install PREFIX=$HOME/.local   # per-user install, no root needed
#   sudo make install PREFIX=/usr      # system-wide
#   make uninstall             # remove installed files
#
# DESTDIR is honored for packaging (e.g. PKGBUILD sets DESTDIR="$pkgdir").
# Also big shoutout to papa claude for helping me with this makefile, genuinely writting random bs and praying that it works.


PREFIX  ?= /usr/local
DESTDIR ?=
CARGO   ?= cargo
APPID   := net.pigmentlab.Pigment
#read the ver straight outta Cargo.toml so it cant drift from the binary again
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

BINDIR     := $(DESTDIR)$(PREFIX)/bin
APPDIR     := $(DESTDIR)$(PREFIX)/share/applications
METADIR    := $(DESTDIR)$(PREFIX)/share/metainfo
ICONBASE   := $(DESTDIR)$(PREFIX)/share/icons/hicolor
ICON_SIZES := 48 64 128 256 512

.PHONY: build install uninstall clean dist

#git archive = only tracked files, no target/ or stray tarballs sneaking in
dist:
	git archive --format=tar.gz --prefix=pigment-$(VERSION)/ -o packaging/pigment-$(VERSION).tar.gz HEAD
	@echo "Wrote packaging/pigment-$(VERSION).tar.gz"

build:
	$(CARGO) build --release --workspace --locked

install: build
	install -Dm755 target/release/pigmentlab      $(BINDIR)/pigmentlab
	install -Dm755 target/release/pigment-launch  $(BINDIR)/pigment-launch
	install -Dm644 packaging/$(APPID).desktop     $(APPDIR)/$(APPID).desktop
	install -Dm644 packaging/$(APPID).metainfo.xml $(METADIR)/$(APPID).metainfo.xml
	@for s in $(ICON_SIZES); do \
		install -Dm644 packaging/icons/$${s}x$${s}/$(APPID).png \
			$(ICONBASE)/$${s}x$${s}/apps/$(APPID).png; \
	done
	@echo
	@echo "Pigment installed to $(PREFIX)."
	@echo "If $(PREFIX)/bin is not on your PATH, add it, then run: pigmentlab"

#per-user uninstall gives roblox:// back first, else links point at a deleted binary.
#skipped for packaging (DESTDIR) n root, whose $$HOME isnt the user who registered
uninstall:
	@if [ -z "$(DESTDIR)" ] && [ "$$(id -u)" != 0 ] && [ -x $(BINDIR)/pigment-launch ]; then \
		$(BINDIR)/pigment-launch --unregister || true; \
	fi
	rm -f $(BINDIR)/pigmentlab
	rm -f $(BINDIR)/pigment-launch
	rm -f $(APPDIR)/$(APPID).desktop
	rm -f $(METADIR)/$(APPID).metainfo.xml
	@for s in $(ICON_SIZES); do rm -f $(ICONBASE)/$${s}x$${s}/apps/$(APPID).png; done

clean:
	$(CARGO) clean
