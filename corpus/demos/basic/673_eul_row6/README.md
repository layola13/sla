# 673 eul_row6

Eulerian 第 6 行 `1,57,302,302,57,1`（行和 `720=6!`；循环求和）。

- 对标：667_eul_row → 低位行互补（同式高位行复算；行和恒等再锁）
- 绕行：行和 `1+57+302+302+57+1=720` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/673_eul_row6/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/673_eul_row6/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/673_eul_row6/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/673_eul_row6/main.sla -o /tmp/basic_673 && /tmp/basic_673
# 期望输出：57,302,720
```
