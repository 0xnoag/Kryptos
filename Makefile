.PHONY: all release sign verify-signature check-hashes hashes audit clean

DAEMON_DIR := src-tauri
RELEASE_DIR := $(DAEMON_DIR)/target/release
DAEMON_BIN := endpoint-privacy-daemon
OUTPUT_DIR := dist

all: release

release: $(OUTPUT_DIR)/$(DAEMON_BIN)-$(VERSION)
	@echo "Release artifacts in $(OUTPUT_DIR)/"

ifndef VERSION
VERSION := $(shell cargo metadata --format-version=1 --no-deps 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])" 2>/dev/null || echo "0.1.0")
endif

$(OUTPUT_DIR)/$(DAEMON_BIN)-$(VERSION): $(DAEMON_DIR)/target/release/$(DAEMON_BIN)
	mkdir -p $(OUTPUT_DIR)
	cp $< $@
	@echo "Built: $@"

$(DAEMON_DIR)/target/release/$(DAEMON_BIN): $(DAEMON_DIR)/src/**/*.rs $(DAEMON_DIR)/Cargo.toml $(DAEMON_DIR)/Cargo.lock
	cargo build --release --manifest-path $(DAEMON_DIR)/Cargo.toml

# GPG-sign the daemon binary (detached ASCII-armored signature)
sign: $(OUTPUT_DIR)/$(DAEMON_BIN)-$(VERSION)
	@test -n "$(GPG_KEY)" || { echo "ERROR: GPG_KEY is not set. Use: make sign GPG_KEY=your@email.com"; exit 1; }
	gpg --detach-sign --armor --default-key "$(GPG_KEY)" \
		--output $(OUTPUT_DIR)/$(DAEMON_BIN)-$(VERSION).asc \
		$(OUTPUT_DIR)/$(DAEMON_BIN)-$(VERSION)
	cd $(OUTPUT_DIR) && sha256sum $(DAEMON_BIN)-$(VERSION) > SHA256SUMS
	gpg --clearsign --default-key "$(GPG_KEY)" $(OUTPUT_DIR)/SHA256SUMS
	@echo "Signed: $(OUTPUT_DIR)/$(DAEMON_BIN)-$(VERSION).asc"
	@echo "Checksums: $(OUTPUT_DIR)/SHA256SUMS.asc"

# Verify GPG signature of a release binary
verify-signature:
	@test -n "$(BINARY)" || { echo "ERROR: BINARY is not set. Use: make verify-signature BINARY=endpoint-privacy-daemon-x.y.z"; exit 1; }
	gpg --verify $(BINARY).asc $(BINARY) 2>/dev/null && echo "SIGNATURE OK" || echo "SIGNATURE MISMATCH"

# Generate .hashes file from installed system binaries for integrity verification.
# Run this on the target system after installing packages to capture current hashes.
hashes:
	@echo "Generating .hashes file from system binaries..."
	@echo "# SHA-256 hashes for external binary integrity verification" > $(OUTPUT_DIR)/hashes.toml
	@echo "# Generated: $$(date -u)" >> $(OUTPUT_DIR)/hashes.toml
	@echo "" >> $(OUTPUT_DIR)/hashes.toml
	@echo "[hashes]" >> $(OUTPUT_DIR)/hashes.toml
	for bin in /usr/bin/tor /usr/bin/obfs4proxy /usr/bin/awg /usr/bin/syncthing; do \
		if [ -f "$$bin" ]; then \
			hash=$$(sha256sum "$$bin" | cut -d' ' -f1); \
			pkg=$$(dpkg -S "$$bin" 2>/dev/null | cut -d: -f1); \
			pkgver=$$(dpkg -l "$$pkg" 2>/dev/null | awk '/^ii/ {print $$3}'); \
			source="$${pkg}=$${pkgver:-unknown}"; \
			echo "\"$$bin\" = { hash = \"$$hash\", source = \"$$source\" }" >> $(OUTPUT_DIR)/hashes.toml; \
		else \
			echo "# $$bin not found (not installed)" >> $(OUTPUT_DIR)/hashes.toml; \
		fi; \
	done
	@echo "Generated: $(OUTPUT_DIR)/hashes.toml"
	@echo "Copy this file to /etc/endpoint-privacy/.hashes on production systems."

# Check current hashes against .hashes file (run on target system)
check-hashes:
	@test -f "$(HASHES)" || { echo "ERROR: HASHES file not found. Set HASHES=/etc/endpoint-privacy/.hashes"; exit 1; }
	@echo "Checking hashes from $(HASHES)..."
	@errors=0; \
	while IFS='=' read -r key rest; do \
		case "$$key" in \
			*"hash"*) \
				path=""; hash="";; \
			*"/usr/bin/"*) \
				path=$$(echo "$$key" | tr -d ' "'); \
				expected=$$(echo "$$rest" | sed 's/.*hash *= *"\([^"]*\)".*/\1/'); \
				if [ -f "$$path" ]; then \
					actual=$$(sha256sum "$$path" | cut -d' ' -f1 | tr '[:lower:]' '[:upper:]'); \
					if [ "$$actual" != "$$expected" ]; then \
						echo "MISMATCH: $$path"; \
						echo "  Expected: $$expected"; \
						echo "  Actual:   $$actual"; \
						errors=$$((errors + 1)); \
					else \
						echo "OK: $$path"; \
					fi; \
				else \
					echo "MISSING: $$path"; \
					errors=$$((errors + 1)); \
				fi;; \
		esac; \
	done < "$(HASHES)"; \
	if [ "$$errors" -gt 0 ]; then \
		echo "FAILED: $$errors hash(es) mismatch or missing"; exit 1; \
	else \
		echo "All hashes match."; \
	fi

# Install system-wide (daemon, launcher, desktop entry, icon)
install: $(DAEMON_DIR)/target/release/$(DAEMON_BIN)
	sudo bash install/install.sh

# Remove system-wide installation
uninstall:
	sudo bash install/uninstall.sh

# Run cargo audit
audit:
	cargo audit --manifest-path $(DAEMON_DIR)/Cargo.toml

clean:
	rm -rf $(OUTPUT_DIR)
	cargo clean --manifest-path $(DAEMON_DIR)/Cargo.toml
