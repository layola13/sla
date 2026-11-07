# 455 duel eulerian

Eulerian 双实现对峙（递推 vs 容斥和；454 的排列版）。

- 对标：454 duel_bell → 容斥 `Σ(-1)^k·C·pow` 锁定（`11=11`）
- 绕行：`pow(0,n)=0` 自然成立（`k=m+1` 末项归零）；奇偶分项加减

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/455_duel_eulerian/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/455_duel_eulerian/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/455_duel_eulerian/main.sla -o /tmp/basic_455 && /tmp/basic_455
# 期望输出：11,11
```
