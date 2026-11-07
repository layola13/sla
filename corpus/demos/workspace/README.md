# SLA Workspace Demo

This demo exercises native `sla` workspace source resolution on top of `sa.mod` workspace manifests.

Build the local CLI once from the plugin root (the build produces `zig-out/bin/sla`):

```bash
cd <sa_plugin_sla repo root>
zig build
```

Then run from this directory (substitute your own checkout paths for `<sci repo>` and `<sa_plugin_sla repo>`):

```bash
PATH=<sci repo>/zig-out/bin:$PATH <sa_plugin_sla repo>/zig-out/bin/sla sla check
PATH=<sci repo>/zig-out/bin:$PATH <sa_plugin_sla repo>/zig-out/bin/sla sla build -p tool
PATH=<sci repo>/zig-out/bin:$PATH <sa_plugin_sla repo>/zig-out/bin/sla sla build-workspace -o app_demo
PATH=<sci repo>/zig-out/bin:$PATH <sa_plugin_sla repo>/zig-out/bin/sla sla build-workspace -p tool -o tool_demo
```

The workspace root defaults to the `app` member. `-p tool` selects the `tool` member without passing a source path.
