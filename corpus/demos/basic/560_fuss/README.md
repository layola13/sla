# 560 fuss

Fuss-Catalan 三元数 `fuss(3,n)=C(3n,n)/(2n+1)`（`n=2→3`，`n=3→12`，`n=4→55`）。

- 对标：351_catalan → 二元商式版互补（此处为三元商式；564 做卷积对峙）
- 绕行：`fuss(1)=1` 退化端点（`C(3,1)/3=1`）；商除数 `2n+1` 恒精确整除

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/560_fuss/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/560_fuss/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/560_fuss/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/560_fuss/main.sla -o /tmp/basic_560 && /tmp/basic_560
# 期望输出：3,12,55
```
