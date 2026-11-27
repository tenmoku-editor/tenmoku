#!/usr/bin/env nu
# A tour of Nushell, for the highlighter.

use std/log
use std/assert [equal "not equal"]

const LIMIT: filesize = 10kb

module greet {
    # Say hello to someone.
    export def hello [name: string = "world"]: nothing -> string {
        $"hello ($name)"
    }

    export-env { $env.GREETED = true }
}

# List the files over a size.
def big-files [
    path: path = "."       # Where to look.
    --min(-m): filesize    # The smallest size to keep.
    --all(-a)
    ...rest: string
]: nothing -> table<name: string, size: filesize> {
    let min = $min | default $LIMIT
    ls --all=$all $path
    | where type == file and size > $min
    | sort-by size --reverse
    | select name size
}

mut total = 0
mut seen: list<string> = []
let config = {a: 1, b: "two", "c d": [1 2 3], ok: true, none: null}
let double = {|x| $x * 2 + 1 }
let doubled = $config."c d" | each $double

for n in 1..<10 {
    if $n mod 2 == 0 {
        $total += $n
    } else if $n >= 7 {
        break
    } else {
        continue
    }
}

while $total > 100 { $total -= 10 }

match $total {
    0 => { print "nothing" }
    1..9 | 11 => "few"
    $x if $x > 50 => $'many: ($x)'
    _ => { log info $"total: ($total * 2)\n" }
}

let raw = r#'C:\no\escapes "here"'#
let escaped = "tab\there \u{1F600}"
let timeout = 2min + 500ms
let mask = 0x[ff 00 1a]
let day = 2026-11-27

$env.PATH = ($env.PATH | prepend "~/.cargo/bin")
$env.config.show_banner = false

try {
    open data.json | get items.0?.name | str replace --regex '^\s+(\w+)$' '$1'
} catch {|err|
    error make {msg: $err.msg}
}

if $raw =~ '[A-Z]:\\\w+' { overlay use greet as hi }
^git log --oneline -n 5 | lines | enumerate | each {|it| $it.index }
nu -c "ls | length"
$in | into int | $in + 1 out> result.txt
