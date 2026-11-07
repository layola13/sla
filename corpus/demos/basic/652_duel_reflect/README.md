# 652 duel_reflect

Ballot 建表 vs 差式对峙（`(4,2)：9 对 9`；`(5,3)：28 对 28`）。

- 对标：646_ballot → 建表源互补（651 差式源合流；跨算法双边锁定）
- 绕行：差式 `15-6=9`、`56-28=28` 正数差（436 口径）；建表上三角守卫同 646

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/652_duel_reflect/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/652_duel_reflect/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/652_duel_reflect/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/652_duel_reflect/main.sla -o /tmp/basic_652 && /tmp/basic_652
# 期望输出：9,9,28,28
```
