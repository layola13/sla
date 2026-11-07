# 891 delan_sym8

Delannoy 续对称性：`D(8,2)=D(2,8)=145`，`D(8,5)=D(5,8)=13073`（镜像双算一致）。

- 对标：886_delan8 → 单格版互补（行列镜像互锁，761 体例续高位）
- 绕行：对称恒等 `D(m,n)=D(n,m)` 双组连验（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/891_delan_sym8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/891_delan_sym8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/891_delan_sym8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/891_delan_sym8/main.sla -o /tmp/basic_891 && /tmp/basic_891
# 期望输出：145,145,13073,13073
```
