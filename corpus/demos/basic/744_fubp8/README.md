# 744 fubp8

有序 Bell 表版高位 `F(8)=545835`（`F(7)=47293` 低位锚；同式再上一位）。

- 对标：742_fubp → `n=7` 版互补（同式再上一位；745 做三向收束）
- 绕行：表版八项和手算复核（探针三线绿；递归深度止 `8`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/744_fubp8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/744_fubp8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/744_fubp8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/744_fubp8/main.sla -o /tmp/basic_744 && /tmp/basic_744
# 期望输出：47293,545835
```
