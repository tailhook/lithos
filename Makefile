RUSTC ?= rustc
CC ?= gcc
AR ?= ar

PREFIX ?= /usr
DESTDIR ?=

LITHOSLIB = liblithos.rlib
QUIRELIB = rust-quire/libquire.rlib
ARGPARSELIB = rust-argparse/libargparse.rlib
REGEXLIB = regex/libregex.rlib

all: bin

test: lithos_test
	./lithos_test

bin:
	cargo build


install:
	cargo build --release
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 target/release/lithos_tree $(DESTDIR)$(PREFIX)/bin/lithos_tree
	install -m 755 target/release/lithos_knot $(DESTDIR)$(PREFIX)/bin/lithos_knot
	install -m 755 target/release/lithos_check $(DESTDIR)$(PREFIX)/bin/lithos_check
	install -m 755 target/release/lithos_clean $(DESTDIR)$(PREFIX)/bin/lithos_clean
	install -m 755 target/release/lithos_cmd $(DESTDIR)$(PREFIX)/bin/lithos_cmd
	install -m 755 target/release/lithos_switch $(DESTDIR)$(PREFIX)/bin/lithos_switch
	install -m 755 target/release/lithos_ps $(DESTDIR)$(PREFIX)/bin/lithos_ps
	install -m 755 bin/lithos_mkdev $(DESTDIR)$(PREFIX)/bin/lithos_mkdev


.PHONY: all bin install test
