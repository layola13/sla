# 423 duel sphenic

楔形数双实现对峙（去重+无平方 vs 计重+去重；394 的形态版）。

- 对标：394 duel_catalan → 双路径锁定（`30 → 1=1`，`60 → 0`）
- 绕行：B 路计重 4 先淘汰 60；嵌套 `while` 内层不加 `;`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/423_duel_sphenic/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/423_duel_sphenic/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/423_duel_sphenic/main.sla -o /tmp/basic_423 && /tmp/basic_423
# 期望输出：1,1,0
```
