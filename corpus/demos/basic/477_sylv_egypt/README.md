# 477 sylv egypt

Sylvester 埃及和（`1805/1806`；457 的倒数版）。

- 对标：457 sylvester → 逐项通分累加（分子/分母双函数打包输出）
- 绕行：步步约分防溢出（`1806` 量级安全）；双函数避多返回值

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/477_sylv_egypt/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/477_sylv_egypt/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/477_sylv_egypt/main.sla -o /tmp/basic_477 && /tmp/basic_477
# 期望输出：1805,1806
```
