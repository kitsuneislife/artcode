This demo shows a minimal package with an `Art.toml`, a library `lib.art` that exports a value, and a `main.art` that imports it.

Files:
- `Art.toml` — package manifest (name, version)
- `lib.art` — library module exporting `lib_val`
- `main.art` — example program that imports `lib` and uses `lib_val`

Run with `art run main.art` from the package directory.
