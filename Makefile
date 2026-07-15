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

PREFIX  ?= /usr/local
DESTDIR ?=
CARGO   ?= cargo
APPID   := org.pigment.Pigment
VERSION := 0.1.0

BINDIR     := $(DESTDIR)$(PREFIX)/bin
APPDIR     := $(DESTDIR)$(PREFIX)/share/applications
ICONBASE   := $(DESTDIR)$(PREFIX)/share/icons/hicolor
ICON_SIZES := 48 64 128 256 512

.PHONY: build install uninstall clean dist

# Produce packaging/pigment-$(VERSION).tar.gz for the PKGBUILD:
#   make dist && cd packaging && makepkg -si
dist:
	rm -rf dist/$(APPID)-tree
	mkdir -p dist/pigment-$(VERSION)
	cp -r crates Cargo.toml Cargo.lock Makefile LICENSE README.md packaging dist/pigment-$(VERSION)/
	rm -f dist/pigment-$(VERSION)/packaging/*.tar.gz
	tar czf packaging/pigment-$(VERSION).tar.gz -C dist pigment-$(VERSION)
	rm -rf dist
	@echo "Wrote packaging/pigment-$(VERSION).tar.gz"

build:
	$(CARGO) build --release --workspace

# Note: depends on `build` so `make install` works standalone. Under makepkg,
# build() already ran cargo, so this cargo invocation is a fast no-op.
install: build
	install -Dm755 target/release/pigmentlab      $(BINDIR)/pigmentlab
	install -Dm755 target/release/pigment-launch  $(BINDIR)/pigment-launch
	install -Dm644 packaging/$(APPID).desktop     $(APPDIR)/$(APPID).desktop
	@for s in $(ICON_SIZES); do \
		install -Dm644 packaging/icons/$${s}x$${s}/$(APPID).png \
			$(ICONBASE)/$${s}x$${s}/apps/$(APPID).png; \
	done
	@echo
	@echo "Pigment installed to $(PREFIX)."
	@echo "If $(PREFIX)/bin is not on your PATH, add it, then run: pigmentlab"

uninstall:
	rm -f $(BINDIR)/pigmentlab
	rm -f $(BINDIR)/pigment-launch
	rm -f $(APPDIR)/$(APPID).desktop
	@for s in $(ICON_SIZES); do rm -f $(ICONBASE)/$${s}x$${s}/apps/$(APPID).png; done

clean:
	$(CARGO) clean
