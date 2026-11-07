# 907 bell_seq11

Bell 序列前 12 项和 `820988`（`B(11)=678570`；前缀和循环求和）。

- 对标：906_bell11 → 单值版互补（同式序列复算；前缀和三档连验）
- 绕行：`142418+678570=820988` 累加手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/907_bell_seq11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/907_bell_seq11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/907_bell_seq11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/907_bell_seq11/main.sla -o /tmp/basic_907 && /tmp/basic_907
# 期望输出：820988,142418
```
