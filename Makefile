include /usr/share/dpkg/pkg-info.mk

BUILDDIR?=build

DEB=librust-proxmox-yew-comp-dev_$(DEB_VERSION)_amd64.deb
BUILD_DEB=$(addprefix $(BUILDDIR)/,$(DEB))

all:
	cargo build --target wasm32-unknown-unknown

$(BUILD_DEB): deb

.PHONY: deb
deb:
	rm -rf $(BUILDDIR)
	mkdir $(BUILDDIR)
	echo system >$(BUILDDIR)/rust-toolchain
	rm -f debian/control
	debcargo package \
	  --config "$(PWD)/debian/debcargo.toml" \
	  --changelog-ready --no-overlay-write-back \
	  --directory "$(PWD)/$(BUILDDIR)/proxmox-yew-comp" \
	  "proxmox-yew-comp" "$(DEB_VERSION_UPSTREAM)"
	cd $(BUILDDIR)/proxmox-yew-comp; dpkg-buildpackage -b -uc -us
	cp $(BUILDDIR)/proxmox-yew-comp/debian/control -f debian/control


.PHONY: check
check:
	cargo test

.PHONY: clean
clean:
	cargo clean
	rm -rf $(BUILDDIR) Cargo.lock
	find . -name '*~' -exec rm {} ';'
