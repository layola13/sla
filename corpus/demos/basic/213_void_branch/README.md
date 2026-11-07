# 213 Void Branch

对标 `tsgosa/demos/675_void_branch`（`7,1`）。

- 分支内 void 调用；输出以 build-exe 核对。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/213_void_branch/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/213_void_branch/main.sla
```
