# 827 stir2_row10

Stirling 二类第 10 行 `S(10,5)=42525`，`S(10,6)=22827`，行和 `115975=Bell(10)`。

- 对标：826_stir29 → 单格版互补（同式行复算；830 做桥对峙，835 做三向）
- 绕行：`1701+5·1050=6951` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/827_stir2_row10/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/827_stir2_row10/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/827_stir2_row10/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/827_stir2_row10/main.sla -o /tmp/basic_827 && /tmp/basic_827
# 期望输出：42525,22827,115975
```
