#!/bin/sh
# Builds tenmoku-server for one target and packs it as a release archive, or writes and signs the
# SHA256SUMS over the three archives. See --help, and docs/decisions/j8wvgszah0-remote-development.md
# § 3 and § 4.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)

usage() {
	cat <<EOF
Usage: scripts/build-tenmoku-server.sh --target TRIPLE [--binary PATH] [--version V] [--out DIR]
       scripts/build-tenmoku-server.sh --sums [--signer FILE] [--require-signature] [--version V] [--out DIR]

With --target, builds tenmoku-server for TRIPLE (cargo build --release --locked -p
tenmoku-server --target TRIPLE) and writes DIR/tenmoku-server-<version>-<asset>.tar.gz, with
tenmoku-server at its root (mode 755) and its licence texts in Licenses/. The targets and their
assets:

  x86_64-unknown-linux-musl     linux-x86_64     static, as every Linux build must be
  aarch64-unknown-linux-musl    linux-aarch64    static
  aarch64-apple-darwin          macos-arm64

It refuses a binary that is not for the target's OS and architecture, and a Linux one that is
dynamically linked. No per-archive .sha256 is written: the signed sums file stands for them.

With --sums, writes DIR/tenmoku-server-<version>-SHA256SUMS over the three archives in DIR (all
three must be there) and, when TENMOKU_UPDATE_PRIVATE_KEY is set, signs it with --signer
(sign_update_feed, crates/fastedit/examples/sign_update_feed.rs) into SHA256SUMS.sig. The key goes
to the signer on stdin only, and the public half the signer reports must be
TENMOKU_UPDATE_PUBLIC_KEY, which the client checks the signature with; otherwise no .sig is left.
Without the private key the sums file is left unsigned, which a client never installs from, unless
--require-signature is given (the workflow gives it for a tag push, which publishes): then it
fails and writes nothing.

  --target TRIPLE   build and pack this target
  --binary PATH     pack this binary instead of building one (it is still checked)
  --sums            write and sign the sums file instead
  --signer FILE     the sign_update_feed binary (needed to sign)
  --require-signature
                    with --sums, fail rather than leave the sums unsigned
  --version V       the version in the names, X.Y.Z or X.Y.Z-preview.N (default:
                    Cargo.toml's [workspace.package])
  --out DIR         where the archive or sums go (default: dist)
  -h, --help        this text

  TENMOKU_UPDATE_PRIVATE_KEY   the base64 Ed25519 private key (a 32-byte seed)
  TENMOKU_UPDATE_PUBLIC_KEY    its public half, as the app builds it in

It prints the archive's or the sums file's path last and exits non-zero on any failure.
EOF
}

die() {
	echo "build-tenmoku-server: $*" >&2
	exit 1
}

need_arg() {
	[ $# -ge 2 ] && [ -n "$2" ] || die "$1 needs a value (see --help)"
}

# The key, out of the environment before anything else runs, so only the signer sees it.
private_key=${TENMOKU_UPDATE_PRIVATE_KEY:-}
unset TENMOKU_UPDATE_PRIVATE_KEY

target="" binary="" sums="" signer="" require="" version="" out=dist
while [ $# -gt 0 ]; do
	case "$1" in
	--target) need_arg "$@"; target=$2; shift 2 ;;
	--binary) need_arg "$@"; binary=$2; shift 2 ;;
	--sums) sums=1; shift ;;
	--signer) need_arg "$@"; signer=$2; shift 2 ;;
	--require-signature) require=1; shift ;;
	--version) need_arg "$@"; version=$2; shift 2 ;;
	--out) need_arg "$@"; out=$2; shift 2 ;;
	-h | --help) usage; exit 0 ;;
	*) die "unknown argument: $1 (see --help)" ;;
	esac
done
[ -n "$target" ] || [ -n "$sums" ] || die "give --target TRIPLE or --sums (see --help)"
[ -z "$target" ] || [ -z "$sums" ] || die "give --target or --sums, not both"

if [ -z "$version" ]; then
	version=$(awk '
		/^\[/ { in_pkg = ($0 == "[workspace.package]") }
		in_pkg && /^version[ \t]*=/ { gsub(/.*=[ \t]*"|".*/, ""); print; exit }
	' "$root/Cargo.toml")
	[ -n "$version" ] || die "no [workspace.package] version in $root/Cargo.toml"
fi
# X.Y.Z, or X.Y.Z-preview.N (N without a leading zero), as check-release-version.sh takes.
case "$version" in
*-preview.*) base=${version%%-preview.*} n=${version#*-preview.} ;;
*) base=$version n=0 ;;
esac
case "$base" in
"" | *[!0-9.]* | .* | *. | *..*) die "version must be X.Y.Z or X.Y.Z-preview.N, got '$version'" ;;
esac
case "$n" in
"" | *[!0-9]* | 0?*) die "version must be X.Y.Z or X.Y.Z-preview.N, got '$version'" ;;
esac

assets="linux-x86_64 linux-aarch64 macos-arm64"
mkdir -p "$out"
out=$(cd "$out" && pwd)

sha256() { # sha256 FILE: prints "<hex>  FILE", as sha256sum -c reads it
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1"
	else
		shasum -a 256 "$1"
	fi
}

if [ -n "$sums" ]; then
	[ -z "$binary" ] || die "--binary is for --target"
	name="tenmoku-server-$version-SHA256SUMS"
	rm -f "$out/$name" "$out/$name.sig"
	[ -n "$private_key" ] || [ -z "$require" ] ||
		die "no TENMOKU_UPDATE_PRIVATE_KEY, and --require-signature: $name would be unsigned, and no client installs from it"
	# The || after the subshell turns errexit off inside it, so every step there exits itself.
	(
		cd "$out" || exit 1
		for asset in $assets; do
			archive="tenmoku-server-$version-$asset.tar.gz"
			[ -f "$archive" ] || die "no $archive in $out"
			sha256 "$archive" || die "hashing $archive failed"
		done
	) >"$out/$name.tmp" || { rm -f "$out/$name.tmp"; exit 1; }
	mv "$out/$name.tmp" "$out/$name"
	if [ -z "$private_key" ]; then
		echo "build-tenmoku-server: $name is left unsigned: no TENMOKU_UPDATE_PRIVATE_KEY" >&2
	else
		public_key=${TENMOKU_UPDATE_PUBLIC_KEY:-}
		[ -n "$public_key" ] || die "a private key but no TENMOKU_UPDATE_PUBLIC_KEY: the client would have no key to check the sums with"
		[ -n "$signer" ] || die "--signer is required to sign the sums"
		[ -x "$signer" ] || die "no signer at $signer"
		# printf is a builtin: the key reaches the signer's stdin and no argv.
		signed=$(printf '%s' "$private_key" | "$signer" sign "$out/$name") ||
			{ rm -f "$out/$name.sig"; die "signing $name failed"; }
		if [ "$signed" != "$public_key" ]; then
			rm -f "$out/$name.sig"
			die "the private key's public half is not TENMOKU_UPDATE_PUBLIC_KEY; no signature written"
		fi
		[ -s "$out/$name.sig" ] || die "the signer wrote no $name.sig"
	fi
	echo "$out/$name"
	exit 0
fi

[ -z "$signer" ] || die "--signer is for --sums"
[ -z "$require" ] || die "--require-signature is for --sums"
case "$target" in
x86_64-unknown-linux-musl) asset=linux-x86_64 for='ELF 64-bit LSB *, x86-64, *' ;;
aarch64-unknown-linux-musl) asset=linux-aarch64 for='ELF 64-bit LSB *, ARM aarch64, *' ;;
aarch64-apple-darwin) asset=macos-arm64 for='Mach-O 64-bit executable arm64' ;;
*) die "no release asset for target $target: the targets are x86_64-unknown-linux-musl, aarch64-unknown-linux-musl and aarch64-apple-darwin" ;;
esac

if [ -z "$binary" ]; then
	# The musl targets link musl's libc and crt statically by default; the check below holds
	# them to it.
	(cd "$root" && cargo build --release --locked -p tenmoku-server --bin tenmoku-server --target "$target") ||
		die "cargo build for $target failed"
	binary=${CARGO_TARGET_DIR:-$root/target}/$target/release/tenmoku-server
fi
[ -f "$binary" ] && [ -x "$binary" ] || die "no executable binary at $binary"

described=$(file -b "$binary")
# shellcheck disable=SC2254 # $for is a pattern
case "$described" in
$for) ;;
*) die "$binary is not a $target binary: $described" ;;
esac
case "$asset" in
linux-*)
	# file(1) calls a binary with no PT_DYNAMIC "statically linked" even when it names an
	# interpreter, so that is refused too.
	case "$described" in
	*"dynamically linked"* | *interpreter*) static="" ;;
	*"statically linked"* | *"static-pie linked"*) static=1 ;;
	*) static="" ;;
	esac
	[ -n "$static" ] || die "$binary is not statically linked, as a Linux build must be (old distros, any libc): $described"
	;;
esac

name="tenmoku-server-$version-$asset"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp "$binary" "$work/tenmoku-server"
chmod 755 "$work/tenmoku-server"
# The licence texts the binary's crates need, flat in Licenses/ as the app archives ship them.
mkdir "$work/Licenses"
for licence in LICENSE NOTICE LICENSES/Apache-2.0.txt LICENSES/MIT.txt THIRD_PARTY.md; do
	[ -f "$root/$licence" ] || die "licence missing: $root/$licence"
	cp "$root/$licence" "$work/Licenses/$(basename "$licence")"
done
# COPYFILE_DISABLE: macOS's tar would otherwise add ._ AppleDouble entries for extended attributes.
COPYFILE_DISABLE=1 tar -C "$work" -czf "$out/$name.tar.gz" tenmoku-server Licenses || die "tar failed"
listed=$(tar -tzf "$out/$name.tar.gz" | grep -v -x -e tenmoku-server -e 'Licenses/' -e 'Licenses/[^/]*' || true)
[ -z "$listed" ] || die "$name.tar.gz holds more than tenmoku-server and its licences: $listed"
echo "$out/$name.tar.gz"
