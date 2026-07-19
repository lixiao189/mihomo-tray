# macOS packaging for Mihomo Tray.
#
# Usage:
#   make package                         # native release .app + .dmg (with helpers)
#   make package TARGET=aarch64-apple-darwin
#   make app                             # .app only (helpers included)
#   make help

APP_NAME      := Mihomo Tray
MAIN_BIN      := mihomo-tray
HELPERS       := mihomo-tray-service \
                 mihomo-tray-service-install \
                 mihomo-tray-service-uninstall

# Optional cross / explicit triple, e.g. TARGET=aarch64-apple-darwin
TARGET        ?=

ifeq ($(TARGET),)
RELEASE_DIR   := target/release
CARGO_FLAGS   :=
BUNDLE_FLAGS  := --release
else
RELEASE_DIR   := target/$(TARGET)/release
CARGO_FLAGS   := --target $(TARGET)
BUNDLE_FLAGS  := --release --target $(TARGET)
endif

APP_BUNDLE    := $(RELEASE_DIR)/bundle/osx/$(APP_NAME).app
MACOS_DIR     := $(APP_BUNDLE)/Contents/MacOS
DMG           := $(RELEASE_DIR)/bundle/dmg/$(APP_NAME).dmg

.PHONY: all help build bundle helpers app dmg package check-tools clean

all: package

help:
	@echo "Targets:"
	@echo "  make package [TARGET=triple]  Build release, cargo-bundle, copy helpers, rebuild DMG"
	@echo "  make app     [TARGET=triple]  Same as package but stop after .app (no DMG)"
	@echo "  make build   [TARGET=triple]  cargo build --release --bins"
	@echo "  make clean                    Remove release bundle outputs"
	@echo ""
	@echo "Examples:"
	@echo "  make package"
	@echo "  make package TARGET=aarch64-apple-darwin"

check-tools:
	@command -v cargo >/dev/null || { echo "error: cargo not found"; exit 1; }
	@command -v cargo-bundle >/dev/null || { \
		echo "error: cargo-bundle not found; install with: cargo install cargo-bundle"; \
		exit 1; \
	}
	@command -v hdiutil >/dev/null || { echo "error: hdiutil not found (macOS only)"; exit 1; }

build: check-tools
	cargo build --release --bins $(CARGO_FLAGS)

bundle: build
	cargo bundle $(BUNDLE_FLAGS)

# cargo-bundle only copies the main binary; place helpers next to it.
helpers: bundle
	@test -d "$(MACOS_DIR)" || { echo "error: missing $(MACOS_DIR)"; exit 1; }
	@for bin in $(HELPERS); do \
		src="$(RELEASE_DIR)/$$bin"; \
		test -f "$$src" || { echo "error: missing $$src"; exit 1; }; \
		cp -f "$$src" "$(MACOS_DIR)/$$bin"; \
		chmod +x "$(MACOS_DIR)/$$bin"; \
		echo "copied $$bin -> $(MACOS_DIR)/"; \
	done

app: helpers
	@echo "App: $(APP_BUNDLE)"
	@file "$(MACOS_DIR)/$(MAIN_BIN)"
	@lipo -archs "$(MACOS_DIR)/$(MAIN_BIN)" 2>/dev/null || true
	@ls -la "$(MACOS_DIR)"

# Rebuild DMG from the helper-complete .app (cargo-bundle's DMG is stale).
dmg: helpers
	@mkdir -p "$(dir $(DMG))"
	@rm -f "$(DMG)"
	@stage=$$(mktemp -d); \
	cp -R "$(APP_BUNDLE)" "$$stage/"; \
	ln -sf /Applications "$$stage/Applications"; \
	hdiutil create -volname "$(APP_NAME)" -srcfolder "$$stage" -ov -format UDZO "$(DMG)"; \
	rm -rf "$$stage"
	@echo "DMG: $(DMG)"
	@ls -lh "$(DMG)"

package: dmg
	@echo ""
	@echo "Packaged:"
	@echo "  $(APP_BUNDLE)"
	@echo "  $(DMG)"

clean:
	rm -rf target/release/bundle
	rm -rf target/*/release/bundle
