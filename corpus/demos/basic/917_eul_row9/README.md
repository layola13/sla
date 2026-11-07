# 917 eul_row9

Eulerian 第 9 行 `E(9,4)=156190`，`E(9,5)=88234`，行和 `362880=9!`。

- 对标：916_eul9 → 单格版互补（同式行复算；923 做行和对峙，925 做三向）
- 绕行：对称恒等 `E(9,5)=E(9,3)` 双算一致（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/917_eul_row9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/917_eul_row9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/917_eul_row9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/917_eul_row9/main.sla -o /tmp/basic_917 && /tmp/basic_917
# 期望输出：156190,88234,362880
```
