# 728 duel_cat

Catalan 双实现对峙：卷积 vs 商式（`n=6：132 对 132`；`n=7：429 对 429`）。

- 对标：726_cat_conv6 → 卷积源（727 商式源合流；561 体例续高位，双边同值锁定）
- 绕行：卷积递归 8s 级（探针同形；商式端精确整除，596 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/728_duel_cat/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/728_duel_cat/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/728_duel_cat/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/728_duel_cat/main.sla -o /tmp/basic_728 && /tmp/basic_728
# 期望输出：132,132,429,429
```
