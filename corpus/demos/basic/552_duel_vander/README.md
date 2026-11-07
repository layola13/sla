# 552 duel_vander

Vandermonde 对峙：卷积求和 `==` 查表 `C(r+s,n)`（`(4,4,4)`：`70` 对 `70`；`(3,5,4)`：`70` 对 `70`）。

- 对标：549_vander → 单实现见源（286 系对峙体例；异参同值 `C(8,4)` 双锁）
- 绕行：卷积越界三层守卫（549 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/552_duel_vander/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/552_duel_vander/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/552_duel_vander/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/552_duel_vander/main.sla -o /tmp/basic_552 && /tmp/basic_552
# 期望输出：70,70,70,70
```
