# 501 qr count

二次剩余计数（含 0；499 的计数版）。

- 对标：499 legendre → 双层存在性扫描（`mod 11 → 6`）
- 绕行：`hit` 标志位（代替 `break`，48 口径无标签）；`p²` 量级小素数安全

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/501_qr_count/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/501_qr_count/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/501_qr_count/main.sla -o /tmp/basic_501 && /tmp/basic_501
# 期望输出：4,6,3
```
