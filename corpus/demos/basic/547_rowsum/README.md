# 547 rowsum

二项式行和 `Σ_k C(n,k) = 2^n`（`n=5→32`，`n=8→256`，`n=10→1024`；表求和）。

- 对标：546_pascal → 单格版互补（此处行求和；550 做闭式对峙）
- 绕行：Pascal 首列恒一式（546 同例，已内联修正版）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/547_rowsum/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/547_rowsum/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/547_rowsum/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/547_rowsum/main.sla -o /tmp/basic_547 && /tmp/basic_547
# 期望输出：32,256,1024
```
