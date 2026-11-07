# 192 Result Basic

对标 `tsgosa/demos/1026_catch_arith` 的值语义改写（`3,false`）。

- SLA 无 try/catch（见 275），错误值语义用 `Result<T, E>` 表达：
  此处为 basic 首个 Result demo（`Ok/Err + is_ok + unwrap`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/192_result_basic/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/192_result_basic/main.sla
```
