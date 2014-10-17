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


$(LITHOSLIB): src/*.rs src/*/*.rs libcontainer.a
	$(RUSTC) src/lib.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

lithos_test: $(ARGPARSELIB) $(QUIRELIB) src/*.rs src/*/*.rs libcontainer.a
	$(RUSTC) src/lib.rs --test -g -o $@ \
		-L rust-quire -L rust-argparse -L .

lithos_tree: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_tree.rs libcontainer.a
	$(RUSTC) src/bin/lithos_tree.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

lithos_knot: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_knot.rs libcontainer.a
	$(RUSTC) src/bin/lithos_knot.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L .

container.o: container.c
	$(CC) -c $< -o $@ -D_GNU_SOURCE -std=c99

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


.PHONY: all quire argparse install test
