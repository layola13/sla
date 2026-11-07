# 837 lah_row9

Lah 第 9 行 `L(9,5)=211680`，`L(9,6)=28224`，行和 `4596553`。

- 对标：836_lah9 → 单格版互补（同式行复算；840 做桥对峙，845 做三向）
- 绕行：行和手算误加一则（`4496553` 误 vs 程序 `4596553`，python 独立复核确证；记账）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/837_lah_row9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/837_lah_row9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/837_lah_row9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/837_lah_row9/main.sla -o /tmp/basic_837 && /tmp/basic_837
# 期望输出：211680,28224,4596553
```
