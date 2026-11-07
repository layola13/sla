# 651 ballot_reflect

Ballot 反射差式 `B(n,k)=C(n+k,k)-C(n+k,k-1)`（`B(4,2)=9`，`B(5,3)=28`，`B(5,5)=42`）。

- 对标：646_ballot → 建表版互补（异构差式单锁；655 做三向收束）
- 绕行：`k=0` 提前返回 `1`（`C(n,-1)` 无定义，守卫前置）；差式天然避负数字面量坑（436 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/651_ballot_reflect/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/651_ballot_reflect/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/651_ballot_reflect/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/651_ballot_reflect/main.sla -o /tmp/basic_651 && /tmp/basic_651
# 期望输出：9,28,42
```
