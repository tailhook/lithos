RUSTC ?= rustc
CC ?= gcc
AR ?= ar

PREFIX ?= /usr
DESTDIR ?=

LITHOSLIB = $(shell rustc --crate-file-name src/lib.rs)
QUIRELIB = rust-quire/$(shell rustc --crate-file-name rust-quire/src/lib.rs)
ARGPARSELIB = rust-argparse/$(shell rustc --crate-file-name rust-argparse/argparse/mod.rs)

all: quire argparse lithos_tree lithos_knot
test: lithos_test
	./lithos_test


$(LITHOSLIB): src/*.rs src/*/*.rs
	$(RUSTC) src/lib.rs -g -o $@ \
		-L rust-quire -L rust-argparse

lithos_test: $(ARGPARSELIB) $(QUIRELIB) src/*.rs src/*/*.rs libcontainer.a
	$(RUSTC) src/lib.rs --test -g -o $@ \
		-L rust-quire -L rust-argparse

lithos_tree: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_tree.rs libcontainer.a
	$(RUSTC) src/bin/lithos_tree.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

lithos_knot: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_knot.rs libcontainer.a
	$(RUSTC) src/bin/lithos_knot.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

libcontainer.a: container.c
	$(CC) -c $< -o container.o -D_GNU_SOURCE -std=c99
	$(AR) rcs $@ container.o

quire:
	make -C rust-quire quire-lib

argparse:
	make -C rust-argparse argparse-lib

install:
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 lithos_tree $(DESTDIR)$(PREFIX)/bin/lithos_tree
	install -m 755 lithos_knot $(DESTDIR)$(PREFIX)/bin/lithos_knot


.PHONY: all quire argparse install test
