RUSTC ?= rustc
CC ?= gcc

PREFIX ?= /usr
DESTDIR ?=

QUIRELIB = rust-quire/$(shell rustc --crate-file-name rust-quire/src/lib.rs)
ARGPARSELIB = rust-argparse/$(shell rustc --crate-file-name rust-argparse/argparse/mod.rs)

all: quire argparse lithos_tree lithos_knot

lithos_tree: $(ARGPARSELIB) $(QUIRELIB) src/*.rs src/*/*.rs libcontainer.a
	$(RUSTC) src/bin/lithos_tree.rs -g -o $@ \
		-L rust-quire -L rust-argparse \
		$(if $(NIX_PROFILES_SUPPORT),--cfg nix_profiles,)

lithos_knot: $(ARGPARSELIB) $(QUIRELIB) src/*.rs src/*/*.rs libcontainer.a
	$(RUSTC) src/bin/lithos_knot.rs -g -o $@ \
		-L rust-quire -L rust-argparse \
		$(if $(NIX_PROFILES_SUPPORT),--cfg nix_profiles,)

libcontainer.a: container.c
	$(CC) -c $< -o $@ -D_GNU_SOURCE -std=c99

quire:
	make -C rust-quire quire-lib

argparse:
	make -C rust-argparse argparse-lib

install:
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 lithos_tree $(DESTDIR)$(PREFIX)/bin/lithos_tree


.PHONY: all quire argparse install
