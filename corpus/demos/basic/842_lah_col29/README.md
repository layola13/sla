# 842 lah_col29

Lah 列纵深 `L(7,2)=15120`，`L(8,2)=141120`，`L(9,2)=1451520`（第二列上行链）。

- 对标：836_lah9 → 单格版互补（纵深三值锁定；843 做列对峙）
- 绕行：`7·40320/2=141120` 商式手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/842_lah_col29/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/842_lah_col29/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/842_lah_col29/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/842_lah_col29/main.sla -o /tmp/basic_842 && /tmp/basic_842
# 期望输出：15120,141120,1451520
```
