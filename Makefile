RUSTC ?= rustc
CC ?= gcc
AR ?= ar

PREFIX ?= /usr
DESTDIR ?=

LITHOSLIB = liblithos.rlib
QUIRELIB = rust-quire/libquire.rlib
ARGPARSELIB = rust-argparse/libargparse.rlib

all: quire argparse lithos_tree lithos_knot lithos_check
test: lithos_test
	./lithos_test


$(LITHOSLIB): src/*.rs src/*/*.rs libcontainer.a
	$(RUSTC) src/lib.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

lithos_test: $(ARGPARSELIB) $(QUIRELIB) src/*.rs src/*/*.rs libcontainer.a
	$(RUSTC) src/lib.rs --test -g -o $@ \
		-L rust-quire -L rust-argparse -L .

lithos_tree: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_tree.rs
	$(RUSTC) src/bin/lithos_tree.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

lithos_check: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_check.rs
	$(RUSTC) src/bin/lithos_check.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

lithos_knot: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_knot.rs
	$(RUSTC) src/bin/lithos_knot.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

container.o: container.c
	$(CC) -c $< -o $@ -fPIC -D_GNU_SOURCE -std=c99

libcontainer.a: container.o
	$(AR) rcs $@ $^

quire:
	make -C rust-quire quire-lib

argparse:
	make -C rust-argparse argparse-lib

install:
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 lithos_tree $(DESTDIR)$(PREFIX)/bin/lithos_tree
	install -m 755 lithos_knot $(DESTDIR)$(PREFIX)/bin/lithos_knot
	install -m 755 lithos_check $(DESTDIR)$(PREFIX)/bin/lithos_check
	install -m 755 bin/lithos_mkdev $(DESTDIR)$(PREFIX)/bin/lithos_mkdev


.PHONY: all quire argparse install test
