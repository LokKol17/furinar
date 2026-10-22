Name:           furinar
Version:        1.5.4
Release:        1%{?dist}
Summary:        A light, fast, and beautiful audio player

License:        BSD-3-Clause
URL:            https://github.com/LokKol17/furinar
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  cargo-rpm-macros
BuildRequires:  gcc
BuildRequires:  alsa-lib-devel
BuildRequires:  gtk3-devel

Requires:       alsa-lib
Requires:       gtk3

%description
Furinar is a lightweight audio player inspired by the Hydro Archon.
It opens in milliseconds and uses only 2-10 MB of RAM while covering
everything a real player needs: tags, synced lyrics, multiple libraries,
and native system integration.

%prep
%autosetup -n %{name}-%{version}

%build
cargo_build --bin furinar

%install
install -Dm755 target/release/furinar %{buildroot}%{_bindir}/furinar
install -Dm644 ui/assets/furinar_icon.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/furinar.png
install -Dm644 pkg/furinar.desktop %{buildroot}%{_datadir}/applications/furinar.desktop
install -Dm644 pkg/furinar.metainfo.xml %{buildroot}%{_datadir}/metainfo/dev.furinar.Furinar.metainfo.xml

%files
%license LICENSE
%{_bindir}/furinar
%{_datadir}/icons/hicolor/256x256/apps/furinar.png
%{_datadir}/applications/furinar.desktop
%{_datadir}/metainfo/dev.furinar.Furinar.metainfo.xml

%changelog
* Sun Sep 21 2026 Furinar Dev <dev@furinar.dev> - 1.5.2-1
- Initial release
