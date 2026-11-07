# 611 delannoy_tab

Delannoy 拍扁建表 `D(m,n)=D(m-1,n)+D(m-1,n-1)+D(m,n-1)`（546 范式）。
`D(2,2)=13`，`D(3,3)=63`，`D(2,3)=25`。

- 对标：464_duel_delannoy → 格路版互补（此处为纯建表版；612 做建表/直和对峙）
- 绕行：首行首列留 `1`（546 范式 `t[nn*w]=1` 同式；`D(4,4)=321` 高位单锁）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/611_delannoy_tab/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/611_delannoy_tab/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/611_delannoy_tab/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/611_delannoy_tab/main.sla -o /tmp/basic_611 && /tmp/basic_611
# 期望输出：13,63,25
```
