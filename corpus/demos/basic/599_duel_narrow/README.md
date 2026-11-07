# 599 duel_narrow

Narayana 行和对峙：行和 `==` Catalan 卷积（`n=4`：`14` 对 `14`；`n=5`：`42` 对 `42`）。

- 对标：597_narayana_row × 556_cat_conv → 两源单实现见各源（286 系对峙体例；Catalan 弧互锁）
- 绕行：双源各自独立（534 同例）；细化求和与卷积双视角锁定

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/599_duel_narrow/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/599_duel_narrow/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/599_duel_narrow/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/599_duel_narrow/main.sla -o /tmp/basic_599 && /tmp/basic_599
# 期望输出：14,14,42,42
```
