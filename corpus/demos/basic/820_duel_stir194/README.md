# 820 duel_stir194

Stirling 一类续高位二对峙：建表 vs 递归（`(9,4)：67284 对 67284`；`(9,5)：22449 对 22449`）。

- 对标：816_stir19 → 内格源互补（819 对峙体例内移；双边同值锁定）
- 绕行：`rcs(9,5)` 递归扇出探针同形 8s 级（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/820_duel_stir194/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/820_duel_stir194/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/820_duel_stir194/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/820_duel_stir194/main.sla -o /tmp/basic_820 && /tmp/basic_820
# 期望输出：67284,67284,22449,22449
```
