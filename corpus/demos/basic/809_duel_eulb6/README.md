# 809 duel_eulb6

Eulerian-B 续高位对峙：建表 vs 交错和（`(6,2)：10543 对 10543`；`(6,3)：23548 对 23548`）。

- 对标：806_eulb6 → 高位源互补（682 交错源合流；679 体例续高位，双边同值锁定）
- 绕行：`(6,2)` 交错三项 `15625-5103+21=10543` 手算复核（探针同形三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/809_duel_eulb6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/809_duel_eulb6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/809_duel_eulb6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/809_duel_eulb6/main.sla -o /tmp/basic_809 && /tmp/basic_809
# 期望输出：10543,10543,23548,23548
```
