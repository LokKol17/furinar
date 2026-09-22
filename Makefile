# ============================================================================
# Furinar — Makefile para build e empacotamento
# ============================================================================

VERSION  ?= 0.1.0
NAME      = furinar
BIN       = target/release/$(NAME)
ICON      = ui/assets/furinar_icon.png

# Tools
CARGO     = cargo
DPKG_DEB  = dpkg-deb
RPMBUILD  = rpmbuild

.PHONY: all build release clean install uninstall \
        deb rpm pkg tarball help

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
	cp pkg/install.sh dist/tarball/$(NAME)-$(VERSION)/
	chmod +x dist/tarball/$(NAME)-$(VERSION)/install.sh
	tar -czf dist/$(NAME)-$(VERSION)-linux-x86_64.tar.gz -C dist/tarball $(NAME)-$(VERSION)
	@echo ""
	@echo "  Tarball gerado: dist/$(NAME)-$(VERSION)-linux-x86_64.tar.gz"
	@echo "  Instalar: tar xzf dist/$(NAME)-$(VERSION)-linux-x86_64.tar.gz && cd $(NAME)-$(VERSION) && sudo ./install.sh --from-source"

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
	@echo ""
	@echo "  make help           Mostra esta mensagem"
	@echo ""
