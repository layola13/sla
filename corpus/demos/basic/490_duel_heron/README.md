# 490 duel heron

Heron 双实现对峙（开方 vs 平方回验；434 的几何版）。

- 对标：434 duel_quad → 完全平方回验锁定（`84=84`；非方返 `0-1`）
- 绕行：`0 - 1` 表负哨兵（387 同例）；回验分支覆盖非 Heron 输入类

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/490_duel_heron/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/490_duel_heron/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/490_duel_heron/main.sla -o /tmp/basic_490 && /tmp/basic_490
# 期望输出：84,84
```
