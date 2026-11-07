# 629 duel_motz

Motzkin 双实现对峙：迭代建表 vs 递归（`n=4：9 对 9`；`n=5：21 对 21`）。

- 对标：626_motzkin → 迭代源（递归版互补；286 系对峙体例，双边同值锁定）
- 绕行：递归双基 `rmoz(0)=1`、`rmoz(1)=1`（无默认参数 #8 口径，显式双返回）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/629_duel_motz/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/629_duel_motz/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/629_duel_motz/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/629_duel_motz/main.sla -o /tmp/basic_629 && /tmp/basic_629
# 期望输出：9,9,21,21
```
