# Migration Guide: v0.4 → v0.5

This guide covers breaking changes and new features introduced in v0.5.

## Breaking changes

### New keywords

The following identifiers are now reserved keywords and can no longer be used as variable or function names:

| Keyword     | Purpose |
|-------------|---------|
| `component` | Declares a reactive UI component block |
| `view`      | View sub-block inside `component {}` |
| `state`     | Reactive mutable binding inside a component |
| `prop`      | Read-only binding passed from outside |
| `memo`      | Derived binding recomputed when deps change |
| `ref`       | Non-reactive DOM reference binding |

**How to migrate:**

If you have variables or parameters named any of the above, rename them.

```art
// v0.4 — worked fine
let state = map_new()

// v0.5 — BREAKS: 'state' is now a keyword
// rename to:
let iter_state = map_new()
```

The most common collision is `state` used as a general-purpose variable. Rename to `iter_state`, `app_state`, `current_state`, etc.

### `art doc` output directory

`art doc <path>` now writes HTML to `docs/generated/<name>.html` instead of `docs.html` in the working directory.

## New features

### `component {}` blocks (Bloco B)

Declare reactive UI components directly in `.art` files:

```art
component Counter {
    state count: Int = 0
    prop label: String
    memo doubled: Int = count * 2
    view {
        <div>
            <p>{label}: {doubled}</p>
            <button on:click={increment}>+</button>
        </div>
    }
}
```

### Reactive dependency graph (Bloco D)

The compiler now builds a DAG of `state → memo → view` dependencies. Circular memo dependencies are a compile-time error:

```
error: reactive cycle detected — 'a' -> 'b' -> 'a'
```

### Async scheduler and lifecycle hooks (Bloco E)

The generated JavaScript runtime includes:
- `__schedule(fn)` — batches DOM updates via `queueMicrotask`
- `on_mount(component, fn)` — called after DOM insertion
- `on_destroy(component, fn)` — called before removal
- `on_update(component, fn, deps)` — called when any dep in `deps` changes
- `tick(fn)` — schedules `fn` in the next microtask

### `Deque<T>` stdlib

A double-ended queue is now available as a builtin:

```art
let d = deque_new()
deque_push_back(d, 1)
deque_push_back(d, 2)
deque_push_front(d, 0)
deque_pop_front(d)  // Option.Some(0)
deque_pop_back(d)   // Option.Some(2)
deque_len(d)        // 1
```

### ArtKit v0.1

See `docs/guides/artkit_quickstart.md` for a complete guide to building reactive UIs with ArtKit.

## Cargo workspace changes

A new crate `crates/reactivity` is part of the workspace. It has no public API surface meant for direct use — it is consumed internally by `codegen_js`.
