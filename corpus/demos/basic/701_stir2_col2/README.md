# 701 stir2_col2

Stirling 二类第 2 列即 Mersenne：`S(7,2)=63`，`S(8,2)=127`，`S(9,2)=255`（`2^(n-1)-1`）。

- 对标：696_stir27 → 内格版互补（列恒等复算；702 做列对峙）
- 绕行：`S(n,2)=2^(n-1)-1` 三值连验（391 Mersenne 体例复用）；`S(6,2)=31` 小值守卫

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/701_stir2_col2/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/701_stir2_col2/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/701_stir2_col2/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/701_stir2_col2/main.sla -o /tmp/basic_701 && /tmp/basic_701
# 期望输出：63,127,255
```
