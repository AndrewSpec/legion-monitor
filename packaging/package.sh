#!/usr/bin/env bash
# Build the distributable packages for legion-monitor.
#
#   packaging/package.sh                  # build, then tarball, .deb and .rpm
#   packaging/package.sh --skip-build     # reuse target/release/legion-monitor
#   packaging/package.sh --formats tarball
#   packaging/package.sh --binary /path/to/legion-monitor --version 0.2.0
#
# Artifacts land in dist/ together with SHA256SUMS. The tarball holds the
# stripped binary, the icons, the desktop entry, install.sh, README.md and
# LICENSE; the .deb and .rpm install the same files under /usr.
#
# The binary links against the build host's glibc, so run this on the oldest
# glibc you want to support. The release workflow builds on Ubuntu 22.04
# (glibc 2.35); a local run on openSUSE Tumbleweed needs glibc 2.43.
set -euo pipefail

name=legion-monitor
root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
out=$root/dist
binary=
version=
do_build=1
formats=tarball,deb,rpm
formats_all=1

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

usage() {
    sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
    case $1 in
        --version)
            [ $# -ge 2 ] || die "--version needs a value"
            version=$2
            shift 2
            ;;
        --binary)
            [ $# -ge 2 ] || die "--binary needs a path"
            binary=$2
            shift 2
            ;;
        --out)
            [ $# -ge 2 ] || die "--out needs a directory"
            out=$2
            shift 2
            ;;
        --formats)
            [ $# -ge 2 ] || die "--formats needs a list"
            formats=$2
            formats_all=0
            shift 2
            ;;
        --skip-build)
            do_build=0
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1 (try --help)"
            ;;
    esac
done

cd "$root"

if [ -z "$version" ]; then
    version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
fi
[ -n "$version" ] || die "cannot read the version from Cargo.toml; pass --version"

repository=$(sed -n 's/^repository = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ -n "$repository" ] || repository=https://github.com/AndrewSpec/legion-monitor
maintainer=${MAINTAINER:-AndrewSpec <andrzej.stolarczyk@outlook.com>}

wants() {
    if [ "$formats_all" -eq 1 ]; then
        return 0
    fi
    case ",$formats," in
        *",$1,"*) return 0 ;;
        *) return 1 ;;
    esac
}

case $formats in
    *all*) formats_all=1 ;;
    *tarball*|*deb*|*rpm*) ;;
    *) die "unknown format list: $formats (tarball, deb, rpm or all)" ;;
esac

echo "==> legion-monitor $version"

if [ "$do_build" -eq 1 ] && [ -z "$binary" ]; then
    echo "==> cargo build --locked --release"
    cargo build --locked --release
fi

if [ -z "$binary" ]; then
    binary=$root/target/release/$name
fi
[ -x "$binary" ] || die "missing $binary; build it first (cargo build --locked --release)"

# Symbols are not shipped; strip a copy so the size does not depend on whether
# rpmbuild/brp-strip runs. Already-stripped input is a no-op for strip(1).
strip_if_possible() {
    if command -v strip >/dev/null 2>&1; then
        strip --strip-unneeded "$1" 2>/dev/null || true
    fi
}

# Highest GLIBC_x.y symbol version the binary needs, used for the .deb Depends.
glibc_minimum() {
    readelf --version-info "$1" 2>/dev/null |
        grep -oE 'GLIBC_[0-9]+\.[0-9]+' |
        sed 's/^GLIBC_//' |
        sort -V -u |
        tail -n 1 || true
}

source_date_epoch=${SOURCE_DATE_EPOCH:-}
if [ -z "$source_date_epoch" ]; then
    source_date_epoch=$(git -C "$root" log -1 --format=%ct 2>/dev/null || true)
fi
[ -n "$source_date_epoch" ] || source_date_epoch=$(date +%s)
# dpkg-deb and rpmbuild clamp their own timestamps from this variable.
export SOURCE_DATE_EPOCH=$source_date_epoch

work=$(mktemp -d "${TMPDIR:-/tmp}/legion-monitor-package.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM

bundle=$work/$name-$version
debroot=$work/deb
rpmtop=$work/rpm
mkdir -p "$bundle" "$out"

install -m755 "$binary" "$bundle/$name"
strip_if_possible "$bundle/$name"
install -m644 "$root/assets/icon.png" "$bundle/icon.png"
install -m644 "$root/assets/icon.svg" "$bundle/icon.svg"
install -m755 "$root/packaging/install.sh" "$bundle/install.sh"
install -m644 "$root/packaging/legion-monitor.desktop" "$bundle/legion-monitor.desktop"
install -m644 "$root/README.md" "$bundle/README.md"
install -m644 "$root/LICENSE" "$bundle/LICENSE"

artifacts=()

build_tarball() {
    local file=$out/$name-$version-x86_64-linux-gnu.tar.gz
    echo "==> tarball: $(basename "$file")"
    tar --sort=name \
        --mtime="@$source_date_epoch" \
        --owner=0 --group=0 --numeric-owner \
        --format=gnu \
        -cf - -C "$work" "$name-$version" |
        gzip -9 -n >"$file"
    artifacts+=("$file")
}

build_deb() {
    command -v dpkg-deb >/dev/null 2>&1 ||
        die "dpkg-deb is required for the .deb (install dpkg)"
    local pkg=$debroot
    local docdir=$pkg/usr/share/doc/$name
    local glibc
    glibc=$(glibc_minimum "$bundle/$name")

    install -d "$pkg/DEBIAN" \
        "$pkg/usr/bin" \
        "$pkg/usr/share/applications" \
        "$pkg/usr/share/icons/hicolor/256x256/apps" \
        "$docdir"
    install -m755 "$bundle/$name" "$pkg/usr/bin/$name"
    install -m644 "$bundle/legion-monitor.desktop" \
        "$pkg/usr/share/applications/$name.desktop"
    install -m644 "$bundle/icon.png" \
        "$pkg/usr/share/icons/hicolor/256x256/apps/$name.png"
    install -m644 "$bundle/README.md" "$docdir/README.md"
    install -m644 "$bundle/LICENSE" "$docdir/copyright"

    local depends=ddcutil
    if [ -n "$glibc" ]; then
        depends="$depends, libc6 (>= $glibc)"
    fi
    depends="$depends, libgcc-s1"

    cat >"$pkg/DEBIAN/control" <<EOF
Package: $name
Version: $version
Architecture: amd64
Maintainer: $maintainer
Installed-Size: $(du -sk "$pkg/usr" | cut -f 1)
Depends: $depends
Section: utils
Priority: optional
Homepage: $repository
Description: Control a Lenovo Legion R45w-30 monitor over DDC/CI
 Legion Monitor is an egui desktop application for brightness, input source,
 picture-in-picture and picture-by-picture modes, colour settings and saved
 profiles of a Lenovo Legion R45w-30 monitor. It talks to the monitor through
 ddcutil and the DDC/CI protocol.
EOF

    (cd "$pkg" && find usr -type f -exec md5sum {} + >DEBIAN/md5sums)

    local file=$out/${name}_${version}_amd64.deb
    echo "==> deb: $(basename "$file")"
    dpkg-deb --root-owner-group -Zxz --build "$pkg" "$file" >/dev/null
    artifacts+=("$file")
}

build_rpm() {
    command -v rpmbuild >/dev/null 2>&1 ||
        die "rpmbuild is required for the .rpm (install rpm)"
    install -d "$rpmtop/BUILD" "$rpmtop/BUILDROOT" "$rpmtop/RPMS" \
        "$rpmtop/SOURCES" "$rpmtop/SPECS" "$rpmtop/SRPMS" "$rpmtop/tmp"
    local spec=$rpmtop/SPECS/$name.spec

    cat >"$spec" <<EOF
%global debug_package %{nil}
# Keep the build time and payload mtimes at SOURCE_DATE_EPOCH (rpm >= 4.15).
%global use_source_date_epoch_as_buildtime 1
%global clamp_mtime_to_source_date_epoch 1

Name:           $name
Version:        $version
Release:        1
Summary:        Control a Lenovo Legion R45w-30 monitor over DDC/CI
License:        GPL-3.0-only
URL:            $repository
Requires:       ddcutil

%description
Legion Monitor is an egui desktop application for brightness, input source,
picture-in-picture and picture-by-picture modes, colour settings and saved
profiles of a Lenovo Legion R45w-30 monitor. It talks to the monitor through
ddcutil and the DDC/CI protocol.

%prep
# No sources: Cargo builds the binary before packaging runs.

%build
# Nothing to build; the stripped binary comes from %{stage}.

%install
install -d %{buildroot}%{_bindir}
install -m755 %{stage}/$name %{buildroot}%{_bindir}/$name
install -d %{buildroot}%{_datadir}/applications
install -m644 %{stage}/legion-monitor.desktop %{buildroot}%{_datadir}/applications/legion-monitor.desktop
install -d %{buildroot}%{_datadir}/icons/hicolor/256x256/apps
install -m644 %{stage}/icon.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/$name.png
install -d %{buildroot}%{_docdir}/$name
install -m644 %{stage}/README.md %{stage}/LICENSE %{buildroot}%{_docdir}/$name/

%post
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q %{_datadir}/applications >/dev/null 2>&1 || :
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t %{_datadir}/icons/hicolor >/dev/null 2>&1 || :
fi

%postun
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q %{_datadir}/applications >/dev/null 2>&1 || :
fi

%files
%{_bindir}/$name
%{_datadir}/applications/legion-monitor.desktop
%{_datadir}/icons/hicolor/256x256/apps/$name.png
%dir %{_docdir}/$name
%{_docdir}/$name/README.md
%{_docdir}/$name/LICENSE
EOF

    echo "==> rpm: $name-$version-1"
    if ! rpmbuild -bb "$spec" \
        --define "_topdir $rpmtop" \
        --define "_tmppath $rpmtop/tmp" \
        --define "_buildhost reproducible" \
        --define "stage $bundle" >"$work/rpmbuild.log" 2>&1; then
        cat "$work/rpmbuild.log" >&2
        die "rpmbuild failed"
    fi
    local file
    file=$(find "$rpmtop/RPMS" -name "$name-$version-1.*.rpm" -type f | head -n 1)
    [ -n "$file" ] || die "rpmbuild produced no package; see $work/rpmbuild.log"
    install -m644 "$file" "$out/"
    artifacts+=("$out/$(basename "$file")")
}

if wants tarball; then build_tarball; fi
if wants deb; then build_deb; fi
if wants rpm; then build_rpm; fi

[ ${#artifacts[@]} -gt 0 ] || die "no artifacts were produced"

( cd "$out" && sha256sum "${artifacts[@]##*/}" >SHA256SUMS )

echo
echo "==> artifacts in $out"
ls -lh "$out" | sed 's/^/    /'
