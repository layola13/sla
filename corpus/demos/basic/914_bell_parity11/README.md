# 914 bell_parity11

Bell 奇偶续 Touchard 同余 `B(11) mod 2=0`，`B(12) mod 2=1`（周期 3 续两位）。

- 对标：784_bell_parity → `n=8/9/10` 版互补（`%2` 判奇偶四值连验；无除余算子绕行）
- 绕行：`x-x/2·2` 表奇偶（`%` 不在面，记 README；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/914_bell_parity11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/914_bell_parity11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/914_bell_parity11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/914_bell_parity11/main.sla -o /tmp/basic_914 && /tmp/basic_914
# 期望输出：0,1,1
```
