# 410 harshad

Harshad 数（`18 → 1`，`<=20 → 13`；406 的整除版）。

- 对标：406 dsum → 整除判定计数（1–9 全是，另 10/12/18/20）
- 绕行：`n < 1` 守卫（`dsum(0)=0` 除零坑）；复用 `dsum`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/410_harshad/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/410_harshad/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/410_harshad/main.sla -o /tmp/basic_410 && /tmp/basic_410
# 期望输出：1,0,13
```
