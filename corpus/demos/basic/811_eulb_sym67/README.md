# 811 eulb_sym67

Eulerian-B 续对称性：`B(6,1)=B(6,5)=722`，`B(7,1)=B(7,6)=2179`（镜像双算一致）。

- 对标：806_eulb6 → 单格版互补（行列镜像互锁；681 体例续高位）
- 绕行：镜像对调用各自独立建表（表形 `w` 各异，双边同值锁定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/811_eulb_sym67/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/811_eulb_sym67/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/811_eulb_sym67/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/811_eulb_sym67/main.sla -o /tmp/basic_811 && /tmp/basic_811
# 期望输出：722,722,2179,2179
```
