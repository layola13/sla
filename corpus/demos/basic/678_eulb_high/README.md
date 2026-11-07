# 678 eulb_high

Eulerian-B 高位 `B(5,2)=1682`（`B(5,1)=237` 低位锚；`5·230+7·76`）。

- 对标：676_eulb → 低位版互补（同式高位复算；680 做高位对峙）
- 绕行：`1150+532=1682` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/678_eulb_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/678_eulb_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/678_eulb_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/678_eulb_high/main.sla -o /tmp/basic_678 && /tmp/basic_678
# 期望输出：237,1682
```
