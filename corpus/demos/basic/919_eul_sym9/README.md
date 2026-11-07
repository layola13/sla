# 919 eul_sym9

Eulerian 续对称性：`E(9,1)=E(9,7)=502`，`E(9,2)=E(9,6)=14608`（镜像双算一致）。

- 对标：916_eul9 → 单格版互补（行列镜像互锁，801 体例续高位）
- 绕行：对称恒等 `E(n,m)=E(n,n-1-m)` 双组连验（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/919_eul_sym9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/919_eul_sym9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/919_eul_sym9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/919_eul_sym9/main.sla -o /tmp/basic_919 && /tmp/basic_919
# 期望输出：502,502,14608,14608
```
