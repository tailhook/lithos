RUSTC ?= rustc
CC ?= gcc
AR ?= ar

PREFIX ?= /usr
DESTDIR ?=

LITHOSLIB = liblithos.rlib
QUIRELIB = rust-quire/libquire.rlib
ARGPARSELIB = rust-argparse/libargparse.rlib
REGEXLIB = regex/libregex.rlib

all: quire argparse lithos_tree lithos_knot lithos_check lithos_cmd
all: lithos_switch lithos_ps

test: lithos_test
	./lithos_test


$(LITHOSLIB): src/*.rs libcontainer.a $(REGEXLIB)
	$(RUSTC) src/lib.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L regex -L .

lithos_test: $(ARGPARSELIB) $(QUIRELIB) src/*.rs src/*/*.rs libcontainer.a
	$(RUSTC) src/lib.rs --test -g -o $@ \
		-L rust-quire -L rust-argparse -L regex -L .

lithos_tree: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_tree.rs
	$(RUSTC) src/bin/lithos_tree.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L regex -L .

lithos_check: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_check.rs
	$(RUSTC) src/bin/lithos_check.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L regex -L .

lithos_knot: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_knot.rs
	$(RUSTC) src/bin/lithos_knot.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L regex -L .

lithos_cmd: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_cmd.rs
	$(RUSTC) src/bin/lithos_cmd.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L regex -L .

lithos_switch: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_switch.rs
	$(RUSTC) src/bin/lithos_switch.rs -g -o $@ \
		-L rust-quire -L rust-argparse -L regex -L .

lithos_ps: $(ARGPARSELIB) $(QUIRELIB) $(LITHOSLIB) src/bin/lithos_ps.rs src/ascii.rs
	$(RUSTC) src/bin/lithos_ps.rs -g -o $@ -C opt-level=3 \
		-L rust-quire -L rust-argparse -L regex -L .

container.o: container.c
	$(CC) -c $< -o $@ -fPIC -D_GNU_SOURCE -std=c99

libcontainer.a: container.o
	$(AR) rcs $@ $^

$(QUIRELIB):
	make -C rust-quire libquire.rlib

$(ARGPARSELIB):
	make -C rust-argparse libargparse.rlib

$(REGEXLIB):
	-mkdir ./run 2> /dev/null
	cd regex; HOME=../run cargo build
	cp regex/target/*.rlib $@

install:
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 lithos_tree $(DESTDIR)$(PREFIX)/bin/lithos_tree
	install -m 755 lithos_knot $(DESTDIR)$(PREFIX)/bin/lithos_knot
	install -m 755 lithos_check $(DESTDIR)$(PREFIX)/bin/lithos_check
	install -m 755 lithos_cmd $(DESTDIR)$(PREFIX)/bin/lithos_cmd
	install -m 755 lithos_switch $(DESTDIR)$(PREFIX)/bin/lithos_switch
	install -m 755 lithos_ps $(DESTDIR)$(PREFIX)/bin/lithos_ps
	install -m 755 bin/lithos_mkdev $(DESTDIR)$(PREFIX)/bin/lithos_mkdev


.PHONY: all quire argparse install test
