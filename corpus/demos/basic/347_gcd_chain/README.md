# 347 gcd chain

三元 gcd 链（`gcd(gcd(a,b),c)`；340 恒等式版的链式版）。

- 对标：80/119 gcd 口径 → 三元链
- 绕行：无（纯 `int` 取模循环）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/347_gcd_chain/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/347_gcd_chain/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/347_gcd_chain/main.sla -o /tmp/basic_347 && /tmp/basic_347
# 期望输出：6,20
```
