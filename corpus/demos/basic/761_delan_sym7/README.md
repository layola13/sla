# 761 delan_sym7

Delannoy 续对称性：`D(7,2)=D(2,7)=113`，`D(7,5)=D(5,7)=7183`（镜像双算一致）。

- 对标：756_delan7 → 单格版互补（行列镜像互锁；619 体例续高位）
- 绕行：写前修正一则（异形守卫改回标准探针体，未进首绿统计）；转置对表形 `w` 各异

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/761_delan_sym7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/761_delan_sym7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/761_delan_sym7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/761_delan_sym7/main.sla -o /tmp/basic_761 && /tmp/basic_761
# 期望输出：113,113,7183,7183
```
