include /usr/share/dpkg/pkg-info.mk

BUILDDIR ?= build

DEBS = \
    librust-proxmox-yew-comp+apt-dev_$(DEB_VERSION)_amd64.deb \
    librust-proxmox-yew-comp+dns-dev_$(DEB_VERSION)_amd64.deb \
    librust-proxmox-yew-comp+network-dev_$(DEB_VERSION)_amd64.deb \
    librust-proxmox-yew-comp+rrd-dev_$(DEB_VERSION)_amd64.deb \
    librust-proxmox-yew-comp-dev_$(DEB_VERSION)_amd64.deb \

BUILD_DEBS = $(addprefix $(BUILDDIR)/,$(DEBS))

all:
	cargo build --target wasm32-unknown-unknown

.PHONY: deb
deb: $(BUILD_DEBS)

$(BUILDDIR):
	rm -rf $@ $@.tmp
	mkdir $@.tmp
	echo system >$@.tmp/rust-toolchain
	rm -f debian/control
	debcargo package \
	  --config "$(PWD)/debian/debcargo.toml" \
	  --changelog-ready --no-overlay-write-back \
	  --directory "$(PWD)/$@.tmp/proxmox-yew-comp" \
	  "proxmox-yew-comp" "$(DEB_VERSION_UPSTREAM)"
	mv $@.tmp $@

$(BUILD_DEBS) &: $(BUILDDIR)
	cd $(BUILDDIR)/proxmox-yew-comp; dpkg-buildpackage -b -uc -us
	cp $(BUILDDIR)/proxmox-yew-comp/debian/control -f debian/control

.PHONY: dsc
dsc: $(BUILDDIR)
	cd $(BUILDDIR)/proxmox-yew-comp; dpkg-buildpackage -S -uc -us -d
	cp $(BUILDDIR)/proxmox-yew-comp/debian/control -f debian/control

upload: UPLOAD_DIST ?= $(DEB_DISTRIBUTION)
upload: $(BUILD_DEBS)
	cd $(BUILDDIR); tar cf - $(DEBS) | ssh -X repoman@repo.proxmox.com -- upload --product devel --dist $(UPLOAD_DIST)

.PHONY: check
check:
	cargo test --all-features --all-targets

.PHONY: clean
clean:
	cargo clean
	rm -rf $(BUILDDIR) Cargo.lock
	find . -name '*~' -exec rm {} ';'
