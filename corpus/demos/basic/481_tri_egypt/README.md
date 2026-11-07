# 481 tri egypt

埃及展开三向（首/项数/末；476 的三向版）。

- 对标：476 egypt → 首项计数末项三函数（`3/7 → 3,3,231`）
- 绕行：草稿函数残留即删（432 同例）；三函数各记一角

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/481_tri_egypt/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/481_tri_egypt/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/481_tri_egypt/main.sla -o /tmp/basic_481 && /tmp/basic_481
# 期望输出：3,3,231
```
