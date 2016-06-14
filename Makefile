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


install: bin-release _install

bin-release:
	cargo build --release -v

_install:
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 target/release/lithos_tree $(DESTDIR)$(PREFIX)/bin/lithos_tree
	install -m 755 target/release/lithos_knot $(DESTDIR)$(PREFIX)/bin/lithos_knot
	install -m 755 target/release/lithos_check $(DESTDIR)$(PREFIX)/bin/lithos_check
	install -m 755 target/release/lithos_clean $(DESTDIR)$(PREFIX)/bin/lithos_clean
	install -m 755 target/release/lithos_cmd $(DESTDIR)$(PREFIX)/bin/lithos_cmd
	install -m 755 target/release/lithos_switch $(DESTDIR)$(PREFIX)/bin/lithos_switch
	install -m 755 target/release/lithos_ps $(DESTDIR)$(PREFIX)/bin/lithos_ps
	install -m 755 bin/lithos_mkdev $(DESTDIR)$(PREFIX)/bin/lithos_mkdev

install-systemd:
	install -D ./systemd.service $(DESTDIR)$(PREFIX)/lib/systemd/system/lithos.service

install-upstart:
	install -D ./upstart.conf $(DESTDIR)/etc/init/lithos.conf

ubuntu-packages: version:=$(shell git describe --dirty)
ubuntu-packages: codename:=$(shell lsb_release --codename --short)
ubuntu-packages:
	rm -rf pkg
	rm -rf target/debug/lithos_*
	bulk with-version "$(version)" cargo build --release
	make _install DESTDIR=/work/pkg
	bulk pack --package-version="$(version)+$(codename)1.noinit"
	make install-$(SYSTEM_KIND) DESTDIR=/work/pkg
	bulk pack --package-version="$(version)+$(codename)1"

ubuntu-lithos_check-package: version:=$(shell git describe --dirty)
ubuntu-lithos_check-package:
	-rm -rf pkg
	-rm -rf target/x86_64-unknown-linux-musl/debug/lithos_*
	bulk with-version "$(version)" \
		cargo build --target=x86_64-unknown-linux-musl --bin=lithos_check
	install -D ./target/x86_64-unknown-linux-musl/debug/lithos_check \
		pkg/usr/bin/lithos_check
	bulk pack --config=bulk-check.yaml --package-version="$(version)"


.PHONY: all bin install test _install bin-release install-systemd install-upstart ubuntu-packages ubuntu-lithos_check-package
