# The tenmoku-server bootstrap (remote development decision, section 4). B is
# the install folder, and F is 1 to install even when the server is present.
set -u
fail() { printf "TENMOKU-BOOT error %s\n" "$*"; exit 1; }
C=${XDG_CACHE_HOME:-$HOME/.cache}/tenmoku-server
D=$C/$B S=$C/$B/tenmoku-server P=missing
[ -x "$S" ] && P=present
printf "TENMOKU-BOOT 1 %s %s %s\n" "$(uname -s)" "$(uname -m)" "$P"
if [ "$P" = missing ] || [ "$F" = 1 ]; then
  read -r n h || exit 1
  e=$(mkdir -p "$D" 2>&1) || fail "$e"
  t=$D/.upload-$$
  e=$(head -c "$n" 2>&1 >"$t.tgz") || { rm -f "$t.tgz"; fail "$e"; }
  s=$(sha256sum "$t.tgz" 2>/dev/null || shasum -a 256 "$t.tgz" 2>/dev/null) ||
    { rm -f "$t.tgz"; fail "found neither sha256sum nor shasum"; }
  s=${s%% *}
  [ "$s" = "$h" ] || { rm -f "$t.tgz"; fail "the upload has SHA-256 $s, not $h"; }
  e=$(mkdir "$t" 2>&1 && tar -xzmf "$t.tgz" -C "$t" 2>&1) && [ -f "$t/tenmoku-server" ] ||
    { rm -rf "$t" "$t.tgz"; fail "can not unpack the upload: ${e:-it has no tenmoku-server}"; }
  e=$(chmod 755 "$t/tenmoku-server" 2>&1 && mv -f "$t/tenmoku-server" "$S" 2>&1) ||
    { rm -rf "$t" "$t.tgz"; fail "$e"; }
  rm -rf "$t" "$t.tgz"
  # The build the server reports names its daemon lock.
  rm -f "$D/build"; v=$("$S" --version 2>/dev/null)
  case $v in "tenmoku-server "?*) printf "%s\n" "${v#tenmoku-server }" >"$D/build" ;; esac
  # Keep the newest three installs (ties go to the later name), any with a
  # daemon lock (sh can not show one free), and any whose build is unknown.
  for o in "$C"/*/tenmoku-server; do
    d=${o%/tenmoku-server}
    [ -f "$o" ] && [ "$d" != "$D" ] && [ -f "$d/build" ] || continue
    k=0 a=0
    for x in "$C"/*/tenmoku-server; do
      if [ "$x" = "$o" ]; then a=1
      elif [ "$x" -nt "$o" ] || { [ "$a" = 1 ] && ! [ "$o" -nt "$x" ]; }; then k=$((k + 1)); fi
    done
    [ "$k" -ge 3 ] || continue
    i= && read -r i <"$d/build"
    [ -n "$i" ] || continue
    l=0
    r=${XDG_RUNTIME_DIR:-/nonexistent}/tenmoku
    for f in "$r/$i.lock" "$HOME/.cache/tenmoku-server/$i/run/$i.lock" /tmp/tenmoku-server-*/"$i.lock"; do
      [ -f "$f" ] && l=1
    done
    [ "$l" = 1 ] || rm -rf "$d"
  done
fi
printf "TENMOKU-BOOT ready\n"
exec "$S" --stdio
