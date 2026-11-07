# 721 nar_sym

Narayana 对称性：`N(7,2)=N(7,4)=105`，`N(8,1)=N(8,6)=28`（镜像双算一致）。

- 对标：716_nar7 → 单格版互补（行列镜像互锁；601 对称体例）
- 绕行：镜像恒等 `N(n,k)=N(n,n-1-k)` 双边同值锁定（商式天然对称）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/721_nar_sym/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/721_nar_sym/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/721_nar_sym/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/721_nar_sym/main.sla -o /tmp/basic_721 && /tmp/basic_721
# 期望输出：105,105,28,28
```
