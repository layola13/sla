# 778 bell10

Bell 续高位二 `B(10)=115975`（`B(9)=21147` 低位锚；十项和 `94828+21147`）。

- 对标：776_bell9 → `n=9` 版互补（同式再上一位；780 做高位对峙）
- 绕行：十项和手算复核（探针三线绿；递归深度止 `10`，两后端秒级）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/778_bell10/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/778_bell10/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/778_bell10/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/778_bell10/main.sla -o /tmp/basic_778 && /tmp/basic_778
# 期望输出：21147,115975
```
