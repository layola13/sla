# 521 tri classify

丰亏完三向计数（`<=20 → 16,3,1`；435 的分类版）。

- 对标：435 tri_constellation → 一函数三 `kind` 分支（`16+3+1=20` 闭合）
- 绕行：`kind` 整型分派（无枚举 `==` 坑，#25 口径）；三 `if` 独立

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/521_tri_classify/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/521_tri_classify/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/521_tri_classify/main.sla -o /tmp/basic_521 && /tmp/basic_521
# 期望输出：16,3,1
```
