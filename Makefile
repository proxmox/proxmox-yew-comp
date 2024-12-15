PKG_VER != dpkg-parsechangelog -l ${PWD}/debian/changelog -SVersion | sed -e 's/-.*//'

all:
	cargo build --target wasm32-unknown-unknown

.PHONY: deb
deb:
	rm -rf build
	mkdir build
	echo system >build/rust-toolchain
	debcargo package \
	  --config "${PWD}/debian/debcargo.toml" \
	  --changelog-ready --no-overlay-write-back \
	  --directory "${PWD}/build/proxmox-yew-comp" \
	  "proxmox-yew-comp" "${PKG_VER}"
	cd build/proxmox-yew-comp; dpkg-buildpackage -b -uc -us


.PHONY: check
check:
	cargo test

.PHONY: clean
clean:
	cargo clean
	rm -rf build Cargo.lock
	find . -name '*~' -exec rm {} ';'
