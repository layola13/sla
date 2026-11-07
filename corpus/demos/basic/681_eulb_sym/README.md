# 681 eulb_sym

Eulerian-B 对称性：`B(4,1)=B(4,3)=76`，`B(5,1)=B(5,4)=237`（镜像双算一致）。

- 对标：676_eulb → 单格版互补（行列镜像互锁；619 对称体例）
- 绕行：镜像对调用各自独立建表（表形 `w` 各异，双边同值锁定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/681_eulb_sym/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/681_eulb_sym/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/681_eulb_sym/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/681_eulb_sym/main.sla -o /tmp/basic_681 && /tmp/basic_681
# 期望输出：76,76,237,237
```
