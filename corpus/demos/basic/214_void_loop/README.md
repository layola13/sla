# 214 Void Loop

对标 `tsgosa/demos/676_void_loop`（`7,7,1`）。

- 循环内 void 调用；输出以 build-exe 核对。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/214_void_loop/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/214_void_loop/main.sla
```
