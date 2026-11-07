# 885 tri_motz11

Motzkin 续三向二：迭代 × 直和 × 桥序和（`n=11：5798` 三锁）。

- 对标：876_motz11 → 单格源互补（三实现互锁；755 三向体例收束 Motzkin 续三行族）
- 绕行：三实现皆迭代/直和/桥序，避递归扇出（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/885_tri_motz11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/885_tri_motz11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/885_tri_motz11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/885_tri_motz11/main.sla -o /tmp/basic_885 && /tmp/basic_885
# 期望输出：5798,5798,5798
```
