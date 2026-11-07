# 869 fubseq9

有序 Bell 序列前缀和 `F(0)+…+F(9)=7685706`（737 体例续高位）。

- 对标：866_fub9 → 单值版互补（同式序列复算；前缀和三档连验）
- 绕行：`52610+545835+7087261=7685706` 累加手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/869_fubseq9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/869_fubseq9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/869_fubseq9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/869_fubseq9/main.sla -o /tmp/basic_869 && /tmp/basic_869
# 期望输出：7685706,598445
```
