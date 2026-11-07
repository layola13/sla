# 808 eulb7

Eulerian-B 第 7 行高位 `B(7,1)=2179`，`B(7,2)=60657`，`B(7,3)=259723`（同式再上一位）。

- 对标：806_eulb6 → `n=6` 版互补（同式再上一位；810 做高位对峙）
- 绕行：`7·23548+9·10543=259723` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/808_eulb7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/808_eulb7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/808_eulb7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/808_eulb7/main.sla -o /tmp/basic_808 && /tmp/basic_808
# 期望输出：2179,60657,259723
```
