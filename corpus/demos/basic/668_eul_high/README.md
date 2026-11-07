# 668 eul_high

Eulerian 高位 `E(6,2)=302`（`E(6,1)=57` 低位锚；`4·26+3·66`）。

- 对标：666_eulerian → 低位版互补（同式高位复算；670 做高位对峙）
- 绕行：`104+198=302` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/668_eul_high/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/668_eul_high/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/668_eul_high/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/668_eul_high/main.sla -o /tmp/basic_668 && /tmp/basic_668
# 期望输出：57,302
```
