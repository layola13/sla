# 671 eul_sym

Eulerian 对称性：`E(5,1)=E(5,3)=26`，`E(6,1)=E(6,4)=57`（镜像双算一致）。

- 对标：666_eulerian → 单格版互补（行列镜像互锁；619 对称体例）
- 绕行：镜像对调用各自独立建表（表形 `w` 各异，双边同值锁定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/671_eul_sym/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/671_eul_sym/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/671_eul_sym/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/671_eul_sym/main.sla -o /tmp/basic_671 && /tmp/basic_671
# 期望输出：26,26,57,57
```
