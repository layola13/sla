# 801 eul_sym78

Eulerian 续对称性：`E(7,1)=E(7,5)=120`，`E(8,1)=E(8,6)=247`（镜像双算一致）。

- 对标：796_eul7 → 单格版互补（行列镜像互锁；671 体例续高位）
- 绕行：镜像对调用各自独立建表（表形 `w` 各异，双边同值锁定）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/801_eul_sym78/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/801_eul_sym78/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/801_eul_sym78/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/801_eul_sym78/main.sla -o /tmp/basic_801 && /tmp/basic_801
# 期望输出：120,120,247,247
```
