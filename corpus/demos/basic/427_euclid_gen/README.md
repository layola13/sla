# 427 euclid gen

Euclid 公式生成（`(2,1) → 30405`；339 的公式版）。

- 对标：339 pyth_gen → `a=m²-n², b=2mn, c=m²+n²` 打包断言
- 绕行：无（万/百位打包；纯 `int` 运算）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/427_euclid_gen/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/427_euclid_gen/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/427_euclid_gen/main.sla -o /tmp/basic_427 && /tmp/basic_427
# 期望输出：30405,51213
```
