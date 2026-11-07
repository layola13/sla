# 69 Len Sum

对标 `tsgosa/demos/196_str_len_sum`（`len("hello")+len("world")=10`）。

- 字面量与变量两种 `len` 路径均已修复（见 `POTENTIAL_ISSUES.md` #10），各取一条断言。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/69_len_sum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/69_len_sum/main.sla
```
