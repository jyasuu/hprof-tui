# hprof-tui

**Terminal UI for Java/Android HPROF heap dump analysis.**

Powered by the [HeapLens](https://github.com/sachinkg12/heaplens) production Rust engine:
two-phase CSR parser · Lengauer-Tarjan dominator tree · HeapQL · waste detection.

---

## Requirements

- **Rust ≥ 1.80** (required by `rayon` via `hprof-analyzer`)
- The `heaplens-main/` directory must sit alongside `hprof-tui/`

```
project-root/
├── hprof-tui/              ← this project
└── heaplens-main/          ← from heaplens-main.zip
    └── hprof-analyzer/
```

## Build & Run

```bash
cd hprof-tui
cargo build --release

# Full analysis (Phase 1 + Phase 2 dominators)
./target/release/hprof-tui /path/to/heap.hprof

# Fast startup — histogram/waste only, no retained sizes
./target/release/hprof-tui --phase1-only /path/to/heap.hprof
```

### Generate a heap dump

```bash
# Running JVM
jmap -dump:format=b,file=heap.hprof <pid>

# On OutOfMemoryError (JVM flag)
-XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=./heap.hprof

# Android (adb)
adb shell am dumpheap <pid> /data/local/tmp/heap.hprof
adb pull /data/local/tmp/heap.hprof
```

---

## Tabs

| # | Tab | What it shows |
|---|-----|---------------|
| 1 | Overview | Heap stats + top-15 retained bar chart + issue badges |
| 2 | Histogram | All classes: shallow + retained sizes, sortable |
| 3 | Retained | Classes ranked by retained size + per-class overhead |
| 4 | Leak Suspects | Objects retaining >10% of heap (from HeapLens engine) |
| 5 | Waste | Duplicate strings · empty collections · over-allocated arrays · boxed primitives |
| 6 | Dominator Tree | Interactive drill-down — Enter to go deeper, Esc to go back |
| ? | Help | Full keybinding reference |

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `→` / `l` | Next tab |
| `Shift+Tab` / `←` / `h` | Previous tab |
| `1`–`6` | Jump to tab |
| `↑↓` / `jk` | Scroll one row |
| `PgDn/PgUp` / `du` | Scroll 10 rows |
| `g` / `Home` | Jump to top |
| `s` | Toggle histogram sort: retained ↔ shallow |
| `Enter` | Dominator Tree: drill into selected object |
| `Esc` / `Backspace` | Dominator Tree: go back to parent |
| `q` / `Ctrl-C` | Quit |
| `?` | Help |

---

## Architecture

```
hprof-tui (this project)
  └── hprof-analyzer (HeapLens engine, Apache 2.0)
        ├── parse_indexed_phase1()   — nodes, class index, GC roots (~1s on 14 GB)
        ├── parse_indexed_phase2()   — edges, Lengauer-Tarjan dominators
        ├── IndexedAnalysisState     — full query interface
        │     ├── get_class_histogram()   → retained-sorted Vec<ClassHistogramEntry>
        │     ├── get_leak_suspects()     → Vec<LeakSuspect>
        │     ├── get_waste_analysis()    → WasteAnalysis (dup strings, empty colls…)
        │     ├── get_children(oid)       → dominator tree children
        │     ├── gc_root_path(oid, n)    → GC root path
        │     └── execute_query(heapql)   → HeapQL results
        └── Phase1AnalysisState      — fast mode (--phase1-only), no dominators
```

## License

Apache 2.0 — same as HeapLens.
