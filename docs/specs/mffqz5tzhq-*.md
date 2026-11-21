
## Implementation notes

The no_gpui lint keeps cfg(unix) out of extensions.rs, so the raw execute bits come from a new kiln_core::platform::execute_bits (0 off Unix), in kiln-core/src/platform/fs.rs and mod.rs: a scope expansion the review findings (F1: any single bit; F2: no PATHEXT off Unix) required.
