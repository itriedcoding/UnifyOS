# UnifyOS — top-level build entry points.
#
# `make system` builds the Rust system layer, `make iso-desktop` bootstraps a
# root filesystem and assembles a bootable hybrid ISO. See documentation/build.md.

SHELL := /bin/bash
VERSION := $(shell cat VERSION)
EDITION ?= desktop
OUT ?= out

# Toolchain is kept inside the repository so builds are reproducible and do not
# depend on a system-wide Rust installation.
CARGO_HOME ?= $(CURDIR)/build/toolchain/cargo
RUSTUP_HOME ?= $(CURDIR)/build/toolchain/rustup
CARGO ?= $(CARGO_HOME)/bin/cargo

.PHONY: all help system test test-system test-installer check-scripts \
        rootfs-desktop rootfs-minimal iso iso-desktop iso-minimal iso-live boot-test checksums clean

all: system

help:
	@echo "UnifyOS $(VERSION) build targets"
	@echo "  make system        build Rust system components (release)"
	@echo "  make test          run Rust unit tests and installer tests"
	@echo "  make rootfs-desktop  bootstrap the desktop root filesystem"
	@echo "  make iso-desktop   build out/UnifyOS-desktop.iso (+ SHA256)"
	@echo "  make iso-minimal   build out/UnifyOS-minimal.iso (+ SHA256)"
	@echo "  make clean         remove build output"

system:
	cd system && CARGO_HOME=$(CARGO_HOME) RUSTUP_HOME=$(RUSTUP_HOME) $(CARGO) build --release

test-system:
	cd system && CARGO_HOME=$(CARGO_HOME) RUSTUP_HOME=$(RUSTUP_HOME) $(CARGO) test --offline

test-installer:
	python3 -m unittest discover -s installer/tests -t installer -v

check-scripts:
	@for script in $$(git ls-files '*.sh' 'iso/**/*.chroot' 2>/dev/null); do \
		bash -n "$$script" || exit 1; \
	done
	@echo "shell syntax OK"

test: test-system test-installer check-scripts

rootfs-desktop:
	./iso/bootstrap-rootfs.sh --edition desktop

rootfs-minimal:
	./iso/bootstrap-rootfs.sh --edition minimal

iso: iso-desktop

iso-desktop: rootfs-desktop
	./iso/make-iso.sh --rootfs build/rootfs-desktop --edition desktop

iso-minimal: rootfs-minimal
	./iso/make-iso.sh --rootfs build/rootfs-minimal --edition minimal

checksums:
	cd $(OUT) && sha256sum *.iso > SHA256SUMS && cat SHA256SUMS

clean:
	rm -rf build/rootfs-desktop build/rootfs-minimal build/iso-desktop build/iso-minimal \
	       system/target $(OUT)

ISO ?= $(OUT)/UnifyOS-$(VERSION)-minimal-amd64.iso

iso-live:
	./iso/build-live.sh

boot-test:
	./scripts/qemu-boot-test.sh $(ISO)
