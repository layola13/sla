# 01 Hello

对标 `tsgosa/demos/01_hello`（`console.log("hello world")`）+ `20_console`。

- `main.sla`：`println("hello world")`，`main -> i32` 返回 0。
- `@test` 用 `str_eq` 锁定字符串语义（等价于 tsgosa 的 `expected.stdout`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/01_hello/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/01_hello/main.sla
```
