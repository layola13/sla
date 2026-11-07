# 52 Pow2 Series

对标 `tsgosa/demos/160_pow2`（`2 ** i`，`i=0..8`，`len/first/last = 8/1/128`）。

- SLA 无 `**` 运算符（`2 ** 3` 会被解析为解引用，探针见 `/tmp/ppow.sla`），
  此处用翻倍 `while` 循环表达；与 `49` 的 `2^10` 标量循环互补，此处落 `Vec`。
- 断言 `len` + 首尾（`Vec<i32>` 写法参考 `39`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/52_pow2_series/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/52_pow2_series/main.sla
```
