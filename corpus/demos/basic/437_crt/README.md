# 437 crt

互素模 CRT（`x=8`；344 的应用版）。

- 对标：344 modinv → 逆元组合同余（`9`、`15` 另两组；余数回验）
- 绕行：`b - a ≥ 0` 选例（避负数取模坑）；复用 344 口径 `modinv`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/437_crt/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/437_crt/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/437_crt/main.sla -o /tmp/basic_437 && /tmp/basic_437
# 期望输出：8,9,15
```
