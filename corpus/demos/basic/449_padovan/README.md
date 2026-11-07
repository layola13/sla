# 449 padovan

Padovan 数（`P(10)=12`；448 的跳阶版）。

- 对标：448 tribonacci → `P(n)=P(n-2)+P(n-3)` 窗口滑动（`P(7)=5`）
- 绕行：`n=0/1/2` 三提前返回（448 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/449_padovan/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/449_padovan/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/449_padovan/main.sla -o /tmp/basic_449 && /tmp/basic_449
# 期望输出：12,5,2
```
