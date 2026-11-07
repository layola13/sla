# 847 nar_row9

Narayana 第 9 行 `N(9,4)=1764`，`N(9,5)=1176`，行和 `4862=Catalan(9)`。

- 对标：846_nar9 → 单格版互补（同式行复算；850 做桥对峙，855 做三向）
- 绕行：`126·126/9=1764` 对角自乘精确整除（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/847_nar_row9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/847_nar_row9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/847_nar_row9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/847_nar_row9/main.sla -o /tmp/basic_847 && /tmp/basic_847
# 期望输出：1764,1176,4862
```
