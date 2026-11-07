# 859 central18

中央二项 `C(18,9)=48620`，`C(20,10)=184756`（Catalan 商式的分子源，同式再上两档）。

- 对标：729_central_binom → 低位版互补（分子源纵深两值锁定；860 做商表对峙）
- 绕行：`C(14,7)=3432` 商式 C(7) 分子连验（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/859_central18/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/859_central18/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/859_central18/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/859_central18/main.sla -o /tmp/basic_859 && /tmp/basic_859
# 期望输出：48620,184756
```
