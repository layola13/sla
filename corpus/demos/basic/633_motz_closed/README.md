# 633 motz_closed

Motzkin 二项直和 `M(n)=sum C(n,2k)·Catalan(k)`（`M(5)=21`，`M(6)=51`，`M(7)=127`）。

- 对标：626_motzkin → 递推版互补（异构直和单锁；635 做三向收束）
- 绕行：`C(2k,k)/(k+1)` 逐项精确整除（596 口径）；`cmot(4)=9` 小值端点守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/633_motz_closed/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/633_motz_closed/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/633_motz_closed/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/633_motz_closed/main.sla -o /tmp/basic_633 && /tmp/basic_633
# 期望输出：21,51,127
```
