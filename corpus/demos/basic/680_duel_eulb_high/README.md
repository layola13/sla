# 680 duel_eulb_high

Eulerian-B 高位对峙：建表 vs 交错和（`(5,1)：237 对 237`；`(5,2)：1682 对 1682`）。

- 对标：678_eulb_high → 高位源互补（679 对峙体例上移；双边同值锁定）
- 绕行：`(5,1)` 交错两项 `243-6=237` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/680_duel_eulb_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/680_duel_eulb_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/680_duel_eulb_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/680_duel_eulb_high/main.sla -o /tmp/basic_680 && /tmp/basic_680
# 期望输出：237,237,1682,1682
```
