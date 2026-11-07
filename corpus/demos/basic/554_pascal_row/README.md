# 554 pascal_row

Pascal 行生成：`C(8,k+1)=C(8,k)·(8-k)/(k+1)` 乘法式逐项整除（350 范式）。
第 8 行：`1,8,28,56,70,56,28,8,1`。

- 对标：350_binom → 单值版互补（此处整行生成；对称侧同断言）
- 绕行：逐项皆精确整除（350 同例）；行号硬编码 8（无默认参数 #8 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/554_pascal_row/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/554_pascal_row/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/554_pascal_row/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/554_pascal_row/main.sla -o /tmp/basic_554 && /tmp/basic_554
# 期望输出：1,8,28,56,70,56,28,8,1
```
