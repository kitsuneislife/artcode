PGO harness

This folder contains a tiny harness to exercise the profile collection + AOT plan generator.

Usage:

```bash
# from repository root
bash bench/run_pgo.sh
```

It will:
- run an example (`cli/examples/12_metrics_demo.art`) to produce `profile.json`
- invoke `art build --with-profile profile.json --out aot_plan.json` which writes a small AOT plan JSON

This is intentionally small and suitable for local experimentation before moving to full AOT build steps.
