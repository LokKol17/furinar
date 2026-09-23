# ============================================================================
# Furinar — Makefile para build e empacotamento
# ============================================================================

VERSION ?= $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
NAME      = furinar
BIN       = target/release/$(NAME)
ICON      = ui/assets/furinar_icon.png

# Tools
CARGO     = cargo
DPKG_DEB  = dpkg-deb
RPMBUILD  = rpmbuild

.PHONY: all build release clean install uninstall \
        deb rpm pkg tarball help sync-version check-version

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

all: build

build:
	$(CARGO) build --release

clean:
	$(CARGO) clean
	rm -rf dist/

# ---------------------------------------------------------------------------
# Install / Uninstall (local)
# ---------------------------------------------------------------------------

install: build
	install -Dm755 $(BIN) $(DESTDIR)/usr/local/bin/$(NAME)
	install -Dm644 $(ICON) $(DESTDIR)/usr/share/icons/hicolor/256x256/apps/$(NAME).png
	install -Dm644 pkg/furinar.desktop $(DESTDIR)/usr/share/applications/$(NAME).desktop
	install -Dm644 pkg/furinar.metainfo.xml $(DESTDIR)/usr/share/metainfo/dev.furinar.$(NAME).metainfo.xml
	@echo ""
	@echo "  Furinar instalado em $(DESTDIR)/usr/local/bin/$(NAME)"

uninstall:
	rm -f $(DESTDIR)/usr/local/bin/$(NAME)
	rm -f $(DESTDIR)/usr/share/icons/hicolor/256x256/apps/$(NAME).png
	rm -f $(DESTDIR)/usr/share/applications/$(NAME).desktop
	rm -f $(DESTDIR)/usr/share/metainfo/dev.furinar.$(NAME).metainfo.xml
	@echo "  Furinar desinstalado."

# ---------------------------------------------------------------------------
# .deb (Debian / Ubuntu)
# ---------------------------------------------------------------------------

deb: build dist/$(NAME)_$(VERSION)_amd64.deb

dist/$(NAME)_$(VERSION)_amd64.deb: build
	@mkdir -p dist/deb/$(NAME)_$(VERSION)_amd64/DEBIAN
	@mkdir -p dist/deb/$(NAME)_$(VERSION)_amd64/usr/bin
	@mkdir -p dist/deb/$(NAME)_$(VERSION)_amd64/usr/share/icons/hicolor/256x256/apps
	@mkdir -p dist/deb/$(NAME)_$(VERSION)_amd64/usr/share/applications
	@mkdir -p dist/deb/$(NAME)_$(VERSION)_amd64/usr/share/metainfo
	cp $(BIN) dist/deb/$(NAME)_$(VERSION)_amd64/usr/bin/$(NAME)
	chmod 755 dist/deb/$(NAME)_$(VERSION)_amd64/usr/bin/$(NAME)
	cp $(ICON) dist/deb/$(NAME)_$(VERSION)_amd64/usr/share/icons/hicolor/256x256/apps/$(NAME).png
	cp pkg/furinar.desktop dist/deb/$(NAME)_$(VERSION)_amd64/usr/share/applications/$(NAME).desktop
	cp pkg/furinar.metainfo.xml dist/deb/$(NAME)_$(VERSION)_amd64/usr/share/metainfo/dev.furinar.$(NAME).metainfo.xml
	cp pkg/debian/control dist/deb/$(NAME)_$(VERSION)_amd64/DEBIAN/control
	sed -i "s/^Version:.*/Version: $(VERSION)/" dist/deb/$(NAME)_$(VERSION)_amd64/DEBIAN/control
	$(DPKG_DEB) --build dist/deb/$(NAME)_$(VERSION)_amd64 dist/$(NAME)_$(VERSION)_amd64.deb
	@echo ""
	@echo "  Pacote gerado: dist/$(NAME)_$(VERSION)_amd64.deb"
	@echo "  Instalar: sudo dpkg -i dist/$(NAME)_$(VERSION)_amd64.deb"

# ---------------------------------------------------------------------------
# .rpm (Fedora / RHEL)
# ---------------------------------------------------------------------------

rpm: build
	@mkdir -p dist/rpm/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
	cp $(BIN) dist/rpm/SOURCES/$(NAME)
	cp $(ICON) dist/rpm/SOURCES/$(NAME).png
	cp pkg/furinar.desktop dist/rpm/SOURCES/$(NAME).desktop
	cp pkg/furinar.metainfo.xml dist/rpm/SOURCES/dev.furinar.$(NAME).metainfo.xml
	cp LICENSE dist/rpm/SOURCES/LICENSE
	$(RPMBUILD) -bb \
		--define "_topdir $(CURDIR)/dist/rpm" \
		--define "_version $(VERSION)" \
		pkg/fedora/$(NAME).spec
	@echo ""
	@echo "  Pacote gerado em dist/rpm/RPMS/"

# ---------------------------------------------------------------------------
# tarball
# ---------------------------------------------------------------------------

tarball: build
	@mkdir -p dist/tarball/$(NAME)-$(VERSION)
	cp $(BIN) dist/tarball/$(NAME)-$(VERSION)/
	cp $(ICON) dist/tarball/$(NAME)-$(VERSION)/$(NAME).png
	cp pkg/furinar.desktop dist/tarball/$(NAME)-$(VERSION)/
	cp pkg/furinar.metainfo.xml dist/tarball/$(NAME)-$(VERSION)/
	cp pkg/install.sh dist/tarball/$(NAME)-$(VERSION)/
	chmod +x dist/tarball/$(NAME)-$(VERSION)/install.sh
	tar -czf dist/$(NAME)-$(VERSION)-linux-x86_64.tar.gz -C dist/tarball $(NAME)-$(VERSION)
	@echo ""
	@echo "  Tarball gerado: dist/$(NAME)-$(VERSION)-linux-x86_64.tar.gz"
	@echo "  Instalar: tar xzf dist/$(NAME)-$(VERSION)-linux-x86_64.tar.gz && cd $(NAME)-$(VERSION) && sudo ./install.sh --from-source"

# ---------------------------------------------------------------------------
# Version sync
# ---------------------------------------------------------------------------

sync-version:
	@echo "  Syncing version from Cargo.toml ($(VERSION)) to all packaging files..."
	@sed -i "s/^Version:.*/Version: $(VERSION)/" pkg/debian/control
	@sed -i "s/^pkgver=.*/pkgver=$(VERSION)/" pkg/arch/PKGBUILD
	@sed -i "s/^Version:.*/Version:        $(VERSION)/" pkg/fedora/furinar.spec
	@sed -i "s/VERSION=\"\$${FURINAR_VERSION:-.*}/VERSION=\"\$${FURINAR_VERSION:-$(VERSION)}/" pkg/install.sh
	@sed -i 's/<release version=".*"/<release version="$(VERSION)"/' pkg/furinar.metainfo.xml
	@echo "  Done. All packaging files updated to $(VERSION)."

check-version:
	@VER="$(VERSION)"; \
		OK=1; \
		DEB_VER=$$(grep '^Version:' pkg/debian/control | sed 's/Version: //'); \
		if [ "$$DEB_VER" != "$$VER" ]; then echo "  MISMATCH: pkg/debian/control has $$DEB_VER, expected $$VER"; OK=0; fi; \
		ARCH_VER=$$(grep '^pkgver=' pkg/arch/PKGBUILD | sed 's/pkgver=//'); \
		if [ "$$ARCH_VER" != "$$VER" ]; then echo "  MISMATCH: pkg/arch/PKGBUILD has $$ARCH_VER, expected $$VER"; OK=0; fi; \
		SPEC_VER=$$(grep '^Version:' pkg/fedora/furinar.spec | sed 's/Version:\s*//'); \
		if [ "$$SPEC_VER" != "$$VER" ]; then echo "  MISMATCH: pkg/fedora/furinar.spec has $$SPEC_VER, expected $$VER"; OK=0; fi; \
		INSTALL_VER=$$(grep 'FURINAR_VERSION' pkg/install.sh | sed 's/.*FURINAR_VERSION:-//;s/".*//'); \
		if [ "$$INSTALL_VER" != "$$VER" ]; then echo "  MISMATCH: pkg/install.sh has $$INSTALL_VER, expected $$VER"; OK=0; fi; \
		META_VER=$$(grep '<release version=' pkg/furinar.metainfo.xml | sed 's/.*version="//;s/".*//'); \
		if [ "$$META_VER" != "$$VER" ]; then echo "  MISMATCH: pkg/furinar.metainfo.xml has $$META_VER, expected $$VER"; OK=0; fi; \
		if [ "$$OK" = "0" ]; then echo ""; echo "  Run 'make sync-version' to fix."; exit 1; fi;
	@echo "  All packaging versions match: $(VERSION)"

# ---------------------------------------------------------------------------
# Help
# ---------------------------------------------------------------------------

help:
	@echo ""
	@echo "  Furinar — Comandos disponíveis:"
	@echo ""
	@echo "  make build          Compila o projeto (release)"
	@echo "  make clean          Remove artefatos de build"
	@echo "  make install        Instala no sistema (/usr/local/bin)"
	@echo "  make uninstall      Remove do sistema"
	@echo ""
	@echo "  make deb            Gera pacote .deb (Debian/Ubuntu)"
	@echo "  make rpm            Gera pacote .rpm (Fedora)"
	@echo "  make tarball        Gera tarball com install.sh"
	@echo "  make release        Gera artefatos para GitHub Releases"
	@echo ""
	@echo "  make sync-version   Sincroniza versão do Cargo.toml com todos os arquivos de packaging"
	@echo "  make check-version  Verifica se todas as versões estão em sync"
	@echo ""
	@echo "  make help           Mostra esta mensagem"
	@echo ""

# ---------------------------------------------------------------------------
# Release artifacts (GitHub Releases)
# ---------------------------------------------------------------------------

release: build
	@mkdir -p dist/release
	@cp $(BIN) dist/release/furinar-linux-x86_64
	@tar -czf dist/release/furinar-linux-x86_64.tar.gz -C target/release $(NAME)
	@cd dist/release && sha256sum * > SHA256SUMS
	@echo ""
	@echo "  Release artifacts in dist/release/"
	@echo "  Upload to GitHub Releases with tag v$(VERSION)"
