# 852 nar_col29

Narayana 列纵深 `N(7,2)=105`，`N(8,2)=196`，`N(9,2)=336`（第二列上行链）。

- 对标：846_nar9 → 单格版互补（纵深三值锁定；853 做列对峙）
- 绕行：`28·56/8=196` 商式手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/852_nar_col29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/852_nar_col29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/852_nar_col29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/852_nar_col29/main.sla -o /tmp/basic_852 && /tmp/basic_852
# 期望输出：105,196,336
```
