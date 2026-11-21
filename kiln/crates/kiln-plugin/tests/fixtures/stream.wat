;; SPDX-License-Identifier: Apache-2.0
;;
;; A plugin that streams programs in background jobs, for `tests/process_stream.rs`. Its
;; interactive instance's `run-command(id, args)`:
;;
;; - `start` calls `process.start` of `idle` in the interactive call, and returns its error (or,
;;   were it to start, drops the child and returns ok);
;; - any other id starts a job named and titled `id`, with `args`, kind `other`, cancellable, no
;;   root; it returns what `jobs.start` returned, without the id.
;;
;; Its worker's `run-job` by name, each failing with the first error an import returned:
;;
;; - `run` starts the program `args` names, reads it to its end, and returns its stdout (at most
;;   2 KiB of it);
;; - `in` starts `show` in the directory `args` names, and does as `run` does;
;; - `cat` starts `cat`, writes `args` to its stdin, closes it, and does as `run` does;
;; - `flood` starts `flood` and then `gate`, reads `gate` to its end without reading `flood`,
;;   then reads `flood` to its end, 1 MiB at a time, and returns how many bytes of stdout it
;;   read, in decimal;
;; - `tree` starts `tree` and reads it until a read fails or it ends, returning `exited` then;
;; - `busy` starts `tree`, then loops without entering the host;
;; - `nine` starts `idle` eight times, then a ninth time: if that starts it returns
;;   `ninth started`; else it drops the first child, starts `idle` once more, and returns the
;;   ninth's error message as its result;
;; - `kill` starts `idle`, kills it, reads it to its end, and returns `k` if the exit says the
;;   host killed it, else `n`;
;; - `huge` starts `idle`, which never reads its stdin, then writes 64 MiB to it in one call over
;;   and over until a write takes nothing, then once more, and returns how many bytes the writes
;;   took in all, in decimal;
;; - `late` starts `gate`, reads it to its end, then does as `in` does.
;;
;; `job-finished` does nothing: the tests take each job's end from the host.
(component $C
  (import "kiln:extension/types@1.1.0" (instance $types
    (type $range (record (field "start" u64) (field "end" u64)))
    (export "range" (type (eq $range)))
    (type $error (variant
      (case "unimplemented" string)
      (case "denied" string)
      (case "not-found" string)
      (case "invalid-argument" string)
      (case "conflict" string)
      (case "failed" string)))
    (export "error" (type (eq $error)))))
  (alias export $types "range" (type $range))
  (alias export $types "error" (type $error))

  (import "kiln:extension/events@1.1.0" (instance $events
    (alias outer $C $range (type $range))
    (type $buffer-opened (record
      (field "buffer" u64)
      (field "version" u64)
      (field "path" (option string))
      (field "language" (option string))))
    (export "buffer-opened" (type $buffer-opened' (eq $buffer-opened)))
    (type $buffer-changed (record
      (field "buffer" u64) (field "version" u64) (field "ranges" (list $range))))
    (export "buffer-changed" (type $buffer-changed' (eq $buffer-changed)))
    (type $buffer-saved (record
      (field "buffer" u64) (field "version" u64) (field "path" string)))
    (export "buffer-saved" (type $buffer-saved' (eq $buffer-saved)))
    (type $selection-changed (record
      (field "buffer" u64) (field "version" u64) (field "ranges" (list $range))))
    (export "selection-changed" (type $selection-changed' (eq $selection-changed)))
    (type $event (variant
      (case "buffer-opened" $buffer-opened')
      (case "buffer-changed" $buffer-changed')
      (case "buffer-saved" $buffer-saved')
      (case "buffer-closed" u64)
      (case "focus-changed" (option u64))
      (case "selection-changed" $selection-changed')
      (case "config-reloaded")
      (case "lagged" u64)))
    (export "event" (type (eq $event)))))
  (alias export $events "event" (type $event))

  (import "kiln:extension/jobs@1.1.0" (instance $jobs
    (alias outer $C $error (type $error))
    (type $job-kind (enum "build" "test" "index" "other"))
    (export "job-kind" (type $job-kind' (eq $job-kind)))
    (type $job-spec (record
      (field "name" string)
      (field "args" string)
      (field "title" string)
      (field "kind" $job-kind')
      (field "cancellable" bool)
      (field "root" (option string))))
    (export "job-spec" (type $job-spec' (eq $job-spec)))
    (type $job-state (variant
      (case "finished" (result string (error $error)))
      (case "cancelled")
      (case "timed-out")))
    (export "job-state" (type (eq $job-state)))
    (export "start" (func (param "spec" $job-spec') (result (result u64 (error $error)))))))
  (alias export $jobs "job-state" (type $job-state))

  (import "kiln:extension/process@1.1.0" (instance $process
    (alias outer $C $error (type $error))
    (export "child" (type $child (sub resource)))
    (type $exit (record (field "code" (option s32)) (field "killed" bool)))
    (export "exit" (type $exit' (eq $exit)))
    (type $chunk (record
      (field "stdout" (list u8))
      (field "stderr" (list u8))
      (field "exited" (option $exit'))))
    (export "chunk" (type $chunk' (eq $chunk)))
    (export "start" (func
      (param "program" string)
      (param "args" (list string))
      (param "cwd" (option string))
      (result (result (own $child) (error $error)))))
    (export "[method]child.read" (func
      (param "self" (borrow $child))
      (param "max-bytes" u32)
      (param "timeout-ms" u32)
      (result (result $chunk' (error $error)))))
    (export "[method]child.write" (func
      (param "self" (borrow $child))
      (param "bytes" (list u8))
      (result (result u64 (error $error)))))
    (export "[method]child.close-stdin" (func (param "self" (borrow $child))))
    (export "[method]child.kill" (func (param "self" (borrow $child))))))
  (alias export $process "child" (type $child))

  (core module $libc
    (memory (export "memory") 1)
    ;; Bytes 0..8 stay zero: a `result` that is `ok`. 64..88 hold `jobs.start`'s result,
    ;; 128..144 `run-command`'s error, 160..176 `run-job`'s result; strings start at 256;
    ;; 400..416 hold `process.start`'s result, 448..484 `read`'s, 512..536 `write`'s; 600..640
    ;; the children of `nine`, 700..720 a number's digits, 8192..10240 the stdout kept. The heap
    ;; starts at 16384, starts there again before each read, and grows the memory as it needs.
    (global $heap (export "heap") (mut i32) (i32.const 16384))
    (func (export "realloc") (param i32 i32) (param $align i32) (param $size i32) (result i32)
      (local $p i32)
      (local $end i32)
      (local $have i32)
      (local.set $p
        (i32.and
          (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
          (i32.sub (i32.const 0) (local.get $align))))
      (local.set $end (i32.add (local.get $p) (local.get $size)))
      (local.set $have (i32.mul (memory.size) (i32.const 65536)))
      (if (i32.gt_u (local.get $end) (local.get $have))
        (then
          (if (i32.eq
                (memory.grow
                  (i32.shr_u
                    (i32.add (i32.sub (local.get $end) (local.get $have)) (i32.const 65535))
                    (i32.const 16)))
                (i32.const -1))
            (then unreachable))))
      (global.set $heap (local.get $end))
      (local.get $p)))
  (core instance $libc (instantiate $libc))
  (alias core export $libc "memory" (core memory $memory))
  (alias core export $libc "realloc" (core func $realloc))

  (alias export $jobs "start" (func $jobs-start))
  (alias export $process "start" (func $process-start))
  (alias export $process "[method]child.read" (func $child-read))
  (alias export $process "[method]child.write" (func $child-write))
  (alias export $process "[method]child.close-stdin" (func $child-close-stdin))
  (alias export $process "[method]child.kill" (func $child-kill))
  (core func $start-job (canon lower (func $jobs-start) (memory $memory) (realloc $realloc)))
  (core func $start (canon lower (func $process-start) (memory $memory) (realloc $realloc)))
  (core func $read (canon lower (func $child-read) (memory $memory) (realloc $realloc)))
  (core func $write (canon lower (func $child-write) (memory $memory) (realloc $realloc)))
  (core func $close-stdin (canon lower (func $child-close-stdin)))
  (core func $kill (canon lower (func $child-kill)))
  (core func $drop (canon resource.drop $child))
  (core instance $host
    (export "start-job" (func $start-job))
    (export "start" (func $start))
    (export "read" (func $read))
    (export "write" (func $write))
    (export "close-stdin" (func $close-stdin))
    (export "kill" (func $kill))
    (export "drop" (func $drop)))

  (core module $main
    (import "libc" "memory" (memory 1))
    (import "libc" "heap" (global $heap (mut i32)))
    (import "host" "start-job" (func $start-job (param i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32 i32)))
    (import "host" "start" (func $start (param i32 i32 i32 i32 i32 i32 i32 i32)))
    (import "host" "read" (func $read (param i32 i32 i32 i32)))
    (import "host" "write" (func $write (param i32 i32 i32 i32)))
    (import "host" "close-stdin" (func $close-stdin (param i32)))
    (import "host" "kill" (func $kill (param i32)))
    (import "host" "drop" (func $drop (param i32)))

    (data (i32.const 256) "idle")
    (data (i32.const 264) "flood")
    (data (i32.const 272) "gate")
    (data (i32.const 280) "tree")
    (data (i32.const 288) "cat")
    (data (i32.const 296) "show")
    (data (i32.const 304) "kn")
    (data (i32.const 312) "ninth started")
    (data (i32.const 328) "exited")

    ;; How much of the stdout read is kept at 8192, and how much was read in all.
    (global $kept (mut i32) (i32.const 0))
    (global $total (mut i64) (i64.const 0))

    ;; `run-job`'s result at 160 as `err` of the error at `at`; returns 160.
    (func $fail (param $at i32) (result i32)
      (i32.store8 (i32.const 160) (i32.const 1))
      (i32.store8 (i32.const 164) (i32.load8_u (local.get $at)))
      (i32.store (i32.const 168) (i32.load offset=4 (local.get $at)))
      (i32.store (i32.const 172) (i32.load offset=8 (local.get $at)))
      (i32.const 160))

    ;; `run-job`'s result at 160 as `ok` of the string at `p`, `n` bytes long; returns 160.
    (func $ok (param $p i32) (param $n i32) (result i32)
      (i32.store8 (i32.const 160) (i32.const 0))
      (i32.store (i32.const 164) (local.get $p))
      (i32.store (i32.const 168) (local.get $n))
      (i32.const 160))

    ;; Starts the program named at `p` (`n` bytes), with no arguments, in the directory at `cp`
    ;; (`cn` bytes) if `has`: its child, or -1 with the error as `run-job`'s result.
    (func $start-in (param $p i32) (param $n i32) (param $has i32) (param $cp i32) (param $cn i32)
      (result i32)
      (call $start
        (local.get $p) (local.get $n)
        (i32.const 0) (i32.const 0)
        (local.get $has) (local.get $cp) (local.get $cn)
        (i32.const 400))
      (if (i32.load8_u (i32.const 400))
        (then (drop (call $fail (i32.const 404))) (return (i32.const -1))))
      (i32.load (i32.const 404)))

    (func $start-program (param $p i32) (param $n i32) (result i32)
      (call $start-in (local.get $p) (local.get $n) (i32.const 0) (i32.const 0) (i32.const 0)))

    ;; Keeps what fits of `n` bytes at `p` after what is kept at 8192.
    (func $keep (param $p i32) (param $n i32)
      (local $room i32)
      (local.set $room (i32.sub (i32.const 2048) (global.get $kept)))
      (if (i32.gt_u (local.get $n) (local.get $room)) (then (local.set $n (local.get $room))))
      (memory.copy (i32.add (i32.const 8192) (global.get $kept)) (local.get $p) (local.get $n))
      (global.set $kept (i32.add (global.get $kept) (local.get $n))))

    ;; Reads `child`, `max` bytes at a time, until it has exited: 1, or 0 with the error as
    ;; `run-job`'s result.
    (func $drain (param $child i32) (param $max i32) (result i32)
      (block $exited
        (loop $more
          (global.set $heap (i32.const 16384))
          (call $read (local.get $child) (local.get $max) (i32.const 1000) (i32.const 448))
          (if (i32.load8_u (i32.const 448))
            (then (drop (call $fail (i32.const 452))) (return (i32.const 0))))
          (global.set $total
            (i64.add (global.get $total) (i64.extend_i32_u (i32.load (i32.const 456)))))
          (call $keep (i32.load (i32.const 452)) (i32.load (i32.const 456)))
          (br_if $exited (i32.load8_u (i32.const 468)))
          (br $more)))
      (i32.const 1))

    ;; `v` in decimal, ending at 720: where it starts.
    (func $decimal (param $v i64) (result i32)
      (local $p i32)
      (local.set $p (i32.const 720))
      (loop $next
        (local.set $p (i32.sub (local.get $p) (i32.const 1)))
        (i32.store8 (local.get $p)
          (i32.add (i32.const 48) (i32.wrap_i64 (i64.rem_u (local.get $v) (i64.const 10)))))
        (local.set $v (i64.div_u (local.get $v) (i64.const 10)))
        (br_if $next (i64.ne (local.get $v) (i64.const 0))))
      (local.get $p))

    (func (export "init") (result i32) (i32.const 0))
    (func (export "shutdown"))
    (func (export "handle-events") (param i32 i32))
    (func (export "job-finished") (param i64 i32 i32 i32 i32 i32))

    (func (export "run-command") (param $id i32) (param $id-len i32) (param $args i32) (param $args-len i32) (result i32)
      ;; start: in this, an interactive call.
      (if (i32.and
            (i32.eq (local.get $id-len) (i32.const 5))
            (i32.eq (i32.load8_u (local.get $id)) (i32.const 115)))
        (then
          (call $start
            (i32.const 256) (i32.const 4)
            (i32.const 0) (i32.const 0)
            (i32.const 0) (i32.const 0) (i32.const 0)
            (i32.const 400))
          (if (i32.eqz (i32.load8_u (i32.const 400)))
            (then (call $drop (i32.load (i32.const 404))) (return (i32.const 0))))
          (i32.store8 (i32.const 128) (i32.const 1))
          (i32.store8 (i32.const 132) (i32.load8_u (i32.const 404)))
          (i32.store (i32.const 136) (i32.load (i32.const 408)))
          (i32.store (i32.const 140) (i32.load (i32.const 412)))
          (return (i32.const 128))))
      ;; Any other: start it.
      (call $start-job
        (local.get $id) (local.get $id-len)
        (local.get $args) (local.get $args-len)
        (local.get $id) (local.get $id-len)
        (i32.const 3) (i32.const 1)
        (i32.const 0) (i32.const 0) (i32.const 0)
        (i32.const 64))
      (if (i32.eqz (i32.load8_u (i32.const 64))) (then (return (i32.const 0))))
      ;; `start`'s error is at 72; `run-command`'s goes at 132.
      (i32.store8 (i32.const 128) (i32.const 1))
      (i32.store8 (i32.const 132) (i32.load8_u (i32.const 72)))
      (i32.store (i32.const 136) (i32.load (i32.const 76)))
      (i32.store (i32.const 140) (i32.load (i32.const 80)))
      (i32.const 128))

    (func (export "run-job")
      (param $job i64) (param $name i32) (param $name-len i32) (param $args i32) (param $args-len i32)
      (param $rooted i32) (param $root i32) (param $root-len i32)
      (result i32)
      (local $first i32)
      (local $child i32)
      (local $gate i32)
      (local $i i32)
      (local $p i32)
      (local $n i32)
      (local $taken i32)
      (local $base i32)
      (local.set $first (i32.load8_u (local.get $name)))
      ;; late: waits for `gate`, then does as `in` does.
      (if (i32.eq (local.get $first) (i32.const 108))
        (then
          (local.set $gate (call $start-program (i32.const 272) (i32.const 4)))
          (if (i32.eq (local.get $gate) (i32.const -1)) (then (return (i32.const 160))))
          (if (i32.eqz (call $drain (local.get $gate) (i32.const 65536)))
            (then (return (i32.const 160))))
          (local.set $first (i32.const 105))))
      ;; huge: 64 MiB at the end of the memory, written until a write takes nothing, then once
      ;; more.
      (if (i32.eq (local.get $first) (i32.const 104))
        (then
          (local.set $child (call $start-program (i32.const 256) (i32.const 4)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (local.set $base (memory.grow (i32.const 1024)))
          (if (i32.eq (local.get $base) (i32.const -1)) (then unreachable))
          (local.set $base (i32.mul (local.get $base) (i32.const 65536)))
          (block $full
            (loop $more
              (call $write (local.get $child) (local.get $base) (i32.const 67108864) (i32.const 512))
              (if (i32.load8_u (i32.const 512)) (then (return (call $fail (i32.const 520)))))
              (local.set $taken (i32.wrap_i64 (i64.load (i32.const 520))))
              (global.set $total (i64.add (global.get $total) (i64.extend_i32_u (local.get $taken))))
              (br_if $full (i32.eqz (local.get $taken)))
              (br $more)))
          (call $write (local.get $child) (local.get $base) (i32.const 67108864) (i32.const 512))
          (if (i32.load8_u (i32.const 512)) (then (return (call $fail (i32.const 520)))))
          (global.set $total
            (i64.add (global.get $total) (i64.extend_i32_u (i32.wrap_i64 (i64.load (i32.const 520))))))
          (local.set $p (call $decimal (global.get $total)))
          (return (call $ok (local.get $p) (i32.sub (i32.const 720) (local.get $p))))))
      ;; run
      (if (i32.eq (local.get $first) (i32.const 114))
        (then
          (local.set $child (call $start-program (local.get $args) (local.get $args-len)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (if (i32.eqz (call $drain (local.get $child) (i32.const 65536)))
            (then (return (i32.const 160))))
          (return (call $ok (i32.const 8192) (global.get $kept)))))
      ;; in
      (if (i32.eq (local.get $first) (i32.const 105))
        (then
          (local.set $child (call $start-in (i32.const 296) (i32.const 4)
            (i32.const 1) (local.get $args) (local.get $args-len)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (if (i32.eqz (call $drain (local.get $child) (i32.const 65536)))
            (then (return (i32.const 160))))
          (return (call $ok (i32.const 8192) (global.get $kept)))))
      ;; cat
      (if (i32.eq (local.get $first) (i32.const 99))
        (then
          (local.set $child (call $start-program (i32.const 288) (i32.const 3)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (local.set $p (local.get $args))
          (local.set $n (local.get $args-len))
          (block $written
            (loop $more
              (br_if $written (i32.eqz (local.get $n)))
              (call $write (local.get $child) (local.get $p) (local.get $n) (i32.const 512))
              (if (i32.load8_u (i32.const 512)) (then (return (call $fail (i32.const 520)))))
              (local.set $taken (i32.wrap_i64 (i64.load (i32.const 520))))
              (local.set $p (i32.add (local.get $p) (local.get $taken)))
              (local.set $n (i32.sub (local.get $n) (local.get $taken)))
              (br $more)))
          (call $close-stdin (local.get $child))
          (if (i32.eqz (call $drain (local.get $child) (i32.const 65536)))
            (then (return (i32.const 160))))
          (return (call $ok (i32.const 8192) (global.get $kept)))))
      ;; flood
      (if (i32.eq (local.get $first) (i32.const 102))
        (then
          (local.set $child (call $start-program (i32.const 264) (i32.const 5)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (local.set $gate (call $start-program (i32.const 272) (i32.const 4)))
          (if (i32.eq (local.get $gate) (i32.const -1)) (then (return (i32.const 160))))
          (if (i32.eqz (call $drain (local.get $gate) (i32.const 65536)))
            (then (return (i32.const 160))))
          (global.set $total (i64.const 0))
          (if (i32.eqz (call $drain (local.get $child) (i32.const 1048576)))
            (then (return (i32.const 160))))
          (local.set $p (call $decimal (global.get $total)))
          (return (call $ok (local.get $p) (i32.sub (i32.const 720) (local.get $p))))))
      ;; tree
      (if (i32.eq (local.get $first) (i32.const 116))
        (then
          (local.set $child (call $start-program (i32.const 280) (i32.const 4)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (if (i32.eqz (call $drain (local.get $child) (i32.const 65536)))
            (then (return (i32.const 160))))
          (return (call $ok (i32.const 328) (i32.const 6)))))
      ;; busy
      (if (i32.eq (local.get $first) (i32.const 98))
        (then
          (local.set $child (call $start-program (i32.const 280) (i32.const 4)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (loop $forever (br $forever))))
      ;; nine
      (if (i32.eq (local.get $first) (i32.const 110))
        (then
          (block $eight
            (loop $more
              (br_if $eight (i32.eq (local.get $i) (i32.const 8)))
              (local.set $child (call $start-program (i32.const 256) (i32.const 4)))
              (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
              (i32.store
                (i32.add (i32.const 600) (i32.shl (local.get $i) (i32.const 2)))
                (local.get $child))
              (local.set $i (i32.add (local.get $i) (i32.const 1)))
              (br $more)))
          (local.set $child (call $start-program (i32.const 256) (i32.const 4)))
          (if (i32.ne (local.get $child) (i32.const -1))
            (then (return (call $ok (i32.const 312) (i32.const 13)))))
          ;; The ninth's message, kept while the first is dropped and one more started.
          (local.set $p (i32.load (i32.const 168)))
          (local.set $n (i32.load (i32.const 172)))
          (call $drop (i32.load (i32.const 600)))
          (local.set $child (call $start-program (i32.const 256) (i32.const 4)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (return (call $ok (local.get $p) (local.get $n)))))
      ;; kill
      (if (i32.eq (local.get $first) (i32.const 107))
        (then
          (local.set $child (call $start-program (i32.const 256) (i32.const 4)))
          (if (i32.eq (local.get $child) (i32.const -1)) (then (return (i32.const 160))))
          (call $kill (local.get $child))
          (if (i32.eqz (call $drain (local.get $child) (i32.const 65536)))
            (then (return (i32.const 160))))
          (if (i32.load8_u (i32.const 480))
            (then (return (call $ok (i32.const 304) (i32.const 1)))))
          (return (call $ok (i32.const 305) (i32.const 1)))))
      unreachable))
  (core instance $main (instantiate $main
    (with "libc" (instance $libc))
    (with "host" (instance $host))))

  (func (export "init") (result (result (error $error)))
    (canon lift (core func $main "init") (memory $memory)))
  (func (export "shutdown")
    (canon lift (core func $main "shutdown")))

  (func $run-command (param "id" string) (param "args" string) (result (result (error $error)))
    (canon lift (core func $main "run-command") (memory $memory) (realloc $realloc)))
  (instance $command-handler (export "run-command" (func $run-command)))
  (export "kiln:extension/command-handler@1.1.0" (instance $command-handler))

  (func $handle-events (param "events" (list $event))
    (canon lift (core func $main "handle-events") (memory $memory) (realloc $realloc)))
  (instance $event-handler (export "handle-events" (func $handle-events)))
  (export "kiln:extension/event-handler@1.1.0" (instance $event-handler))

  (func $run-job (param "job" u64) (param "name" string) (param "args" string) (param "root" (option string))
    (result (result string (error $error)))
    (canon lift (core func $main "run-job") (memory $memory) (realloc $realloc)))
  (instance $job-runner (export "run-job" (func $run-job)))
  (export "kiln:extension/job-runner@1.1.0" (instance $job-runner))

  (func $job-finished (param "job" u64) (param "state" $job-state)
    (canon lift (core func $main "job-finished") (memory $memory) (realloc $realloc)))
  (instance $job-handler (export "job-finished" (func $job-finished)))
  (export "kiln:extension/job-handler@1.1.0" (instance $job-handler)))
