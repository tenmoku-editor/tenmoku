#!/bin/sh
# Channel sets for scripts/velopack-pack.sh --linux and scripts/velopack-pack.ps1 (T1998): the vpk
# argument lists and signer steps of their --dry-run for a stable version (stable, both) and a
# preview one, then a pack through a fake vpk and signer showing which files each leaves. No
# real vpk runs. The .ps1 cases need PowerShell (pwsh, or PWSH=path); without it they are skipped
# with a message. Usage: scripts/test-velopack-pack.sh
set -eu
LC_ALL=C
export LC_ALL
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
sh_pack="$here/velopack-pack.sh"
ps1="$here/velopack-pack.ps1"
t=$(mktemp -d)
trap 'rm -rf "$t"' EXIT
fail() { echo "FAIL: $*"; exit 1; }
ok() { echo "ok: $*"; }

pin=$(sed -n 's/^velopack = { version = "=\([^"]*\)".*/\1/p' "$root/Cargo.toml")
[ -n "$pin" ] || fail "no pinned velopack version in Cargo.toml"
mkdir -p "$t/app/Licenses" "$t/bin"
cp /usr/bin/true "$t/app/tenmoku"
cp /usr/bin/true "$t/app/tenmoku.exe"

# A fake vpk: -h names the pin; pack writes the files vpk writes into --outputDir (an AppImage
# and a Setup.exe holding the channel's name, the channel's feed and full .nupkg) and logs any
# private key it can see.
cat >"$t/bin/vpk" <<EOF
#!/bin/sh
[ "\$1" = -h ] && { echo "Velopack CLI $pin"; exit 0; }
[ "\$1" = '[linux]' ] && shift
[ "\$1" = pack ] || exit 2
env | grep TENMOKU_UPDATE_PRIVATE_KEY >>"$t/vpk-saw-key"
while [ \$# -gt 0 ]; do
	case "\$1" in
	--channel) c=\$2 ;; --outputDir) o=\$2 ;; --packVersion) v=\$2 ;;
	esac
	shift
done
mkdir -p "\$o"
echo "\$c" >"\$o/Tenmoku.AppImage"
echo "\$c" >"\$o/Tenmoku-win-Setup.exe"
echo "{\"Assets\":[\"\$c\"]}" >"\$o/releases.\$c.json"
echo "\$c" >"\$o/Tenmoku-\$v-\$c-full.nupkg"
EOF
# A fake signer: sign FILE reads the key from stdin, writes FILE.sig and prints FAKE_PUBLIC.
cat >"$t/bin/sign" <<EOF
#!/bin/sh
[ "\$1" = sign ] || exit 2
cat >/dev/null
echo sig >"\$2.sig"
echo "\$FAKE_PUBLIC"
EOF
chmod 755 "$t/bin/vpk" "$t/bin/sign"

# shellcheck disable=SC1003 # tr's '\\' is one backslash
# norm FILE: the log with the work directory as WORK, the checkout as ROOT, '\' as '/'.
norm() {
	work=$(tr '\\' '/' <"$1" | sed -n 's/.*--packDir \([^ ]*\)\/app .*/\1/p' | head -n 1)
	tr '\\' '/' <"$1" | awk -v w="$work" -v r="$root" '{
		if (w != "") while ((i = index($0, w)) > 0) $0 = substr($0, 1, i - 1) "WORK" substr($0, i + length(w))
		while ((i = index($0, r)) > 0) $0 = substr($0, 1, i - 1) "ROOT" substr($0, i + length(r))
		print
	}'
}

# --- velopack-pack.sh --linux, dry ---------------------------------------------------------
linux_dry() { env -u TENMOKU_UPDATE_PRIVATE_KEY_FILE VPK=/opt/vpk "$sh_pack" --linux --pack-dir "$t/app" \
	--out /dist --dry-run "$@"; }
cross=""
[ "$(uname -s)" = Linux ] || cross=" '[linux]'"
# linux_vpk V CHANNEL: the vpk command line --linux runs for that channel.
linux_vpk() {
	echo "/opt/vpk$cross pack --packId Tenmoku --packVersion $1 --packDir WORK/app --mainExe tenmoku --channel $2 --runtime linux-x64 --packTitle Tenmoku --packAuthors Tenmoku --icon ROOT/crates/fastedit/assets/icon/fastedit-1024.png --categories 'Development;TextEditor;' --delta None --outputDir WORK/vpk-${2##*-}"
}
signs() { echo "<private key from TENMOKU_UPDATE_PRIVATE_KEY> | /opt/sign sign /dist/releases.$1.json"; }
keys() { TENMOKU_UPDATE_PRIVATE_KEY=secretseed TENMOKU_UPDATE_PUBLIC_KEY=AbCd= "$@"; }

keys linux_dry --version 1.2.3 --signer /opt/sign >"$t/log" 2>&1 || { cat "$t/log"; fail "stable dry run exited non-zero"; }
{ linux_vpk 1.2.3 linux-x86_64-stable; signs linux-x86_64-stable; } >"$t/want"
norm "$t/log" | diff -u "$t/want" - || fail "--linux, stable by default: not one stable pack and signature"
grep -q secretseed "$t/log" && fail "velopack-pack.sh printed the private key"
ok "--linux --version 1.2.3 (default stable): vpk on linux-x86_64-stable, its feed signed"

keys linux_dry --version 1.2.3 --signer /opt/sign --channel-set both >"$t/log" 2>&1 || { cat "$t/log"; fail "both dry run exited non-zero"; }
{ linux_vpk 1.2.3 linux-x86_64-stable; signs linux-x86_64-stable
	linux_vpk 1.2.3 linux-x86_64-preview; signs linux-x86_64-preview; } >"$t/want"
norm "$t/log" | diff -u "$t/want" - || fail "--linux --channel-set both, stable version: not stable then preview, each signed"
ok "--linux --version 1.2.3 --channel-set both: vpk on linux-x86_64-stable and linux-x86_64-preview, each feed signed"

keys linux_dry --version 1.2.3 --signer /opt/sign --channel-set preview >"$t/log" 2>&1 || { cat "$t/log"; fail "preview set dry run exited non-zero"; }
{ linux_vpk 1.2.3 linux-x86_64-preview; signs linux-x86_64-preview; } >"$t/want"
norm "$t/log" | diff -u "$t/want" - || fail "--linux --channel-set preview, stable version: not one preview pack"
ok "--linux --version 1.2.3 --channel-set preview: vpk on linux-x86_64-preview only"

for set in both preview; do
	keys linux_dry --version 1.2.3-preview.2 --signer /opt/sign --channel-set $set >"$t/log" 2>&1 ||
		{ cat "$t/log"; fail "preview version, $set: dry run exited non-zero"; }
	{ linux_vpk 1.2.3-preview.2 linux-x86_64-preview; signs linux-x86_64-preview; } >"$t/want"
	norm "$t/log" | diff -u "$t/want" - || fail "--linux, preview version, --channel-set $set: not one preview pack"
	grep -q stable "$t/log" && fail "a preview version, --channel-set $set, names a stable channel"
done
if keys linux_dry --version 1.2.3-preview.2 --signer /opt/sign >"$t/log" 2>&1; then
	fail "a preview version was packed with the default --channel-set stable"
fi
grep -q 'a preview version goes on the preview channel only' "$t/log" || fail "the stable refusal says: $(cat "$t/log")"
ok "--linux --version 1.2.3-preview.2: linux-x86_64-preview only, with both or preview; refused with stable"

TENMOKU_UPDATE_PRIVATE_KEY='' TENMOKU_UPDATE_PUBLIC_KEY='' linux_dry --version 1.2.3 --channel-set both >"$t/log" 2>&1 ||
	{ cat "$t/log"; fail "unsigned dry run exited non-zero"; }
for c in stable preview; do
	grep -q -x -F "velopack-pack: note: no private key, so releases.linux-x86_64-$c.json is left unsigned" "$t/log" ||
		fail "no unsigned note for $c: $(cat "$t/log")"
done
grep -q ' sign ' "$t/log" && fail "a signer step with no private key"
ok "--linux with no private key: each channel's feed noted unsigned, no signer step"

for v in 1.2.3-preview 1.2.3-preview.01 1.2.3-rc.1 1.2.3-Preview.1 1.2.3-preview.2-preview.3 -preview.1 1.2.; do
	if TENMOKU_UPDATE_PRIVATE_KEY='' linux_dry --version "$v" --channel-set both >"$t/log" 2>&1; then
		fail "--linux took version '$v'"
	fi
done
if TENMOKU_UPDATE_PRIVATE_KEY='' linux_dry --version 1.2.3 --channel-set beta >"$t/log" 2>&1; then
	fail "--linux took --channel-set beta"
fi
ok "--linux refuses versions other than X.Y.Z and X.Y.Z-preview.N, and channel sets other than stable, preview, both"

# --- velopack-pack.sh --linux through the fake vpk -----------------------------------------
# linux_run OUT V SET: a real (not dry) pack into OUT; prints the files left there, sorted.
linux_run() {
	out=$1
	shift
	rm -f "$t/vpk-saw-key"
	keys env FAKE_PUBLIC=AbCd= VPK="$t/bin/vpk" "$sh_pack" --linux --pack-dir "$t/app" --out "$out" \
		--signer "$t/bin/sign" --version "$@" >"$t/log" 2>&1 || { cat "$t/log"; fail "--linux $* exited non-zero"; }
	[ ! -s "$t/vpk-saw-key" ] || fail "vpk saw the private key: $(cat "$t/vpk-saw-key")"
	(cd "$out" && find . -type f | sed 's|^\./||' | sort)
}
linux_run "$t/l-stable" 1.2.3 --channel-set both >"$t/got"
cat >"$t/want" <<'EOF'
Tenmoku-1.2.3-linux-x86_64-preview-full.nupkg
Tenmoku-1.2.3-linux-x86_64-stable-full.nupkg
Tenmoku-1.2.3-linux-x86_64.AppImage
Tenmoku-1.2.3-linux-x86_64.AppImage.sha256
releases.linux-x86_64-preview.json
releases.linux-x86_64-preview.json.sig
releases.linux-x86_64-stable.json
releases.linux-x86_64-stable.json.sig
EOF
diff -u "$t/want" "$t/got" || fail "--linux 1.2.3 both left other files"
[ "$(cat "$t/l-stable/Tenmoku-1.2.3-linux-x86_64.AppImage")" = linux-x86_64-stable ] || fail "the AppImage is not the stable pack's"
(cd "$t/l-stable" && shasum -a 256 -c Tenmoku-1.2.3-linux-x86_64.AppImage.sha256 >/dev/null) || fail "the AppImage's .sha256"
[ "$(sed -n '$p' "$t/log")" = "$t/l-stable/releases.linux-x86_64-preview.json.sig" ] || fail "the last line printed: $(cat "$t/log")"
ok "--linux 1.2.3 both, through a fake vpk: the stable AppImage, both channels' feeds, .nupkg and .sig; vpk never sees the key"

linux_run "$t/l-pre" 1.2.3-preview.2 --channel-set both >"$t/got"
cat >"$t/want" <<'EOF'
Tenmoku-1.2.3-preview.2-linux-x86_64-preview-full.nupkg
Tenmoku-1.2.3-preview.2-linux-x86_64.AppImage
Tenmoku-1.2.3-preview.2-linux-x86_64.AppImage.sha256
releases.linux-x86_64-preview.json
releases.linux-x86_64-preview.json.sig
EOF
diff -u "$t/want" "$t/got" || fail "--linux 1.2.3-preview.2 both left other files"
grep -q stable "$t/got" "$t/log" && fail "a preview version left a stable channel file"
[ "$(cat "$t/l-pre/Tenmoku-1.2.3-preview.2-linux-x86_64.AppImage")" = linux-x86_64-preview ] || fail "the AppImage is not the preview pack's"
ok "--linux 1.2.3-preview.2 both, through a fake vpk: no -stable file, the preview channel's AppImage, feed, .nupkg and .sig"

if keys env FAKE_PUBLIC=abCd= VPK="$t/bin/vpk" "$sh_pack" --linux --pack-dir "$t/app" --out "$t/l-bad" \
	--signer "$t/bin/sign" --version 1.2.3 --channel-set both >"$t/log" 2>&1; then
	fail "--linux took a signer whose public half differs from TENMOKU_UPDATE_PUBLIC_KEY in case only"
fi
ls "$t/l-bad"/*.sig >/dev/null 2>&1 && fail "a .sig was left after a key mismatch"
ok "--linux refuses a key whose public half differs in case only, and leaves no .sig"

# --- velopack-pack.ps1 ---------------------------------------------------------------------
pwsh=${PWSH:-$(command -v pwsh || true)}
if [ -z "$pwsh" ]; then
	echo "skip: PowerShell (pwsh) is not installed, so velopack-pack.ps1's dry run is not checked; install it or set PWSH"
	exit 0
fi
win_dry() { env -u TENMOKU_UPDATE_PRIVATE_KEY_FILE VPK=/opt/vpk "$pwsh" -NoProfile -NonInteractive -File "$ps1" \
	-PackDir "$t/app" -Out /dist -DryRun "$@"; }
# win_vpk ARCH V KIND: the vpk command line the .ps1 runs for channel windows-ARCH-KIND.
win_vpk() {
	echo "/opt/vpk pack --packId Tenmoku --packVersion $2 --packDir WORK/app --mainExe tenmoku.exe --channel windows-$1-$3 --runtime win-$1 --packTitle Tenmoku --packAuthors Tenmoku --icon ROOT/crates/fastedit/assets/windows/tenmoku.ico --shortcuts StartMenuRoot --noPortable --delta None --outputDir WORK/vpk-$3"
}
for arch in x64 arm64; do
	keys win_dry -Arch $arch -Version 1.2.3 -Signer /opt/sign >"$t/log" 2>&1 || { cat "$t/log"; fail "ps1 $arch stable dry run exited non-zero"; }
	{ win_vpk $arch 1.2.3 stable; signs windows-$arch-stable; } >"$t/want"
	norm "$t/log" | diff -u "$t/want" - || fail "ps1 $arch, stable by default: not one stable pack and signature"
	grep -q secretseed "$t/log" && fail "velopack-pack.ps1 printed the private key"

	keys win_dry -Arch $arch -Version 1.2.3 -Signer /opt/sign -ChannelSet both >"$t/log" 2>&1 || { cat "$t/log"; fail "ps1 $arch both exited non-zero"; }
	{ win_vpk $arch 1.2.3 stable; signs windows-$arch-stable; win_vpk $arch 1.2.3 preview; signs windows-$arch-preview; } >"$t/want"
	norm "$t/log" | diff -u "$t/want" - || fail "ps1 $arch -ChannelSet both: not stable then preview, each signed"

	for set in both preview; do
		keys win_dry -Arch $arch -Version 1.2.3-preview.2 -Signer /opt/sign -ChannelSet $set >"$t/log" 2>&1 ||
			{ cat "$t/log"; fail "ps1 $arch preview version, $set: exited non-zero"; }
		{ win_vpk $arch 1.2.3-preview.2 preview; signs windows-$arch-preview; } >"$t/want"
		norm "$t/log" | diff -u "$t/want" - || fail "ps1 $arch, preview version, -ChannelSet $set: not one preview pack"
	done
	if keys win_dry -Arch $arch -Version 1.2.3-preview.2 -Signer /opt/sign >"$t/log" 2>&1; then
		fail "ps1 packed a preview version with the default -ChannelSet stable"
	fi
	grep -q 'a preview version goes on the preview channel only' "$t/log" || fail "the ps1 stable refusal says: $(cat "$t/log")"
done
ok "ps1 dry run, x64 and arm64: stable by default, stable+preview with both, a preview version on windows-<arch>-preview only"

for v in 1.2.3-preview 1.2.3-preview.01 1.2.3-rc.1 1.2.3-Preview.1; do
	if TENMOKU_UPDATE_PRIVATE_KEY='' win_dry -Arch x64 -Version "$v" -ChannelSet both >"$t/log" 2>&1; then
		fail "ps1 took version '$v'"
	fi
done
TENMOKU_UPDATE_PRIVATE_KEY='' TENMOKU_UPDATE_PUBLIC_KEY='' win_dry -Arch x64 -Version 1.2.3 -ChannelSet both >"$t/log" 2>&1 ||
	{ cat "$t/log"; fail "ps1 unsigned dry run exited non-zero"; }
for c in stable preview; do
	grep -q -x -F "velopack-pack: note: no private key, so releases.windows-x64-$c.json is left unsigned" "$t/log" ||
		fail "ps1: no unsigned note for $c: $(cat "$t/log")"
done
ok "ps1 refuses other versions, and notes each feed unsigned with no private key"

# win_run OUT V SET [PUBLIC]: a real (not dry) .ps1 pack through the fake vpk and signer.
win_run() {
	out=$1 v=$2 set=$3
	rm -f "$t/vpk-saw-key"
	keys env FAKE_PUBLIC="${4:-AbCd=}" VPK="$t/bin/vpk" "$pwsh" -NoProfile -NonInteractive -File "$ps1" -Arch x64 \
		-PackDir "$t/app" -Out "$out" -Signer "$t/bin/sign" -Version "$v" -ChannelSet "$set" >"$t/log" 2>&1
}
mkdir -p "$t/w-stable" "$t/w-pre" "$t/w-bad"
win_run "$t/w-stable" 1.2.3 both || { cat "$t/log"; fail "ps1 1.2.3 both through the fake vpk exited non-zero"; }
[ ! -s "$t/vpk-saw-key" ] || fail "ps1: vpk saw the private key"
(cd "$t/w-stable" && find . -type f | sed 's|^\./||' | sort) >"$t/got"
cat >"$t/want" <<'EOF'
Tenmoku-1.2.3-windows-x64-Setup.exe
Tenmoku-1.2.3-windows-x64-Setup.exe.sha256
Tenmoku-1.2.3-windows-x64-preview-full.nupkg
Tenmoku-1.2.3-windows-x64-stable-full.nupkg
releases.windows-x64-preview.json
releases.windows-x64-preview.json.sig
releases.windows-x64-stable.json
releases.windows-x64-stable.json.sig
EOF
diff -u "$t/want" "$t/got" || fail "ps1 1.2.3 both left other files"
[ "$(cat "$t/w-stable/Tenmoku-1.2.3-windows-x64-Setup.exe")" = windows-x64-stable ] || fail "Setup.exe is not the stable pack's"

win_run "$t/w-pre" 1.2.3-preview.2 both || { cat "$t/log"; fail "ps1 preview both through the fake vpk exited non-zero"; }
(cd "$t/w-pre" && find . -type f | sed 's|^\./||' | sort) >"$t/got"
cat >"$t/want" <<'EOF'
Tenmoku-1.2.3-preview.2-windows-x64-Setup.exe
Tenmoku-1.2.3-preview.2-windows-x64-Setup.exe.sha256
Tenmoku-1.2.3-preview.2-windows-x64-preview-full.nupkg
releases.windows-x64-preview.json
releases.windows-x64-preview.json.sig
EOF
diff -u "$t/want" "$t/got" || fail "ps1 1.2.3-preview.2 both left other files"
grep -q stable "$t/got" && fail "ps1: a preview version left a stable channel file"
ok "ps1 through a fake vpk: 1.2.3 both leaves both channels' feeds, .nupkg and .sig; 1.2.3-preview.2 no -stable file"

# T1909 F3: base64 is case-sensitive, so a public half differing in case only is refused.
if win_run "$t/w-bad" 1.2.3 stable abCd=; then
	fail "ps1 took a signer whose public half differs from TENMOKU_UPDATE_PUBLIC_KEY in case only"
fi
grep -q "public half is not TENMOKU_UPDATE_PUBLIC_KEY" "$t/log" || fail "ps1's case-only mismatch says: $(cat "$t/log")"
ls "$t/w-bad"/*.sig >/dev/null 2>&1 && fail "ps1 left a .sig after a key mismatch"
ok "ps1 refuses a key whose public half differs in case only, and leaves no .sig"
