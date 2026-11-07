# 887 delan_row8

Delannoy 第 8 行 `D(8,5)=13073`，`D(8,6)=40081`，行和 `432073`。

- 对标：886_delan8 → 单格版互补（同式行复算；890 做对峙，895 做三向）
- 绕行：`D(8,5)` 手算误加一则（`14073` 误 vs 三源 `13073`，直和逐项复算确证；记账）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/887_delan_row8/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/887_delan_row8/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/887_delan_row8/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/887_delan_row8/main.sla -o /tmp/basic_887 && /tmp/basic_887
# 期望输出：13073,40081,432073
```
