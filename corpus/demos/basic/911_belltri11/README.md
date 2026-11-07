# 911 belltri11

Bell Aitken 三角续高位 `B(11)=678570`，`B(12)=4213597`（781 体例续高位）。

- 对标：906_bell11 → 递推版互补（三角纵深四值锁定；915 做三向收束）
- 绕行：三角行末即 Bell 数（`T(n,0)=T(n-1,n-1)`；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/911_belltri11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/911_belltri11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/911_belltri11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/911_belltri11/main.sla -o /tmp/basic_911 && /tmp/basic_911
# 期望输出：678570,4213597,115975
```
