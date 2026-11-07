# 258 Pow Sqmul

对标 `tsgosa/demos/1231_pow` 的平方乘版（49/113 为循环连乘版：`1024,243,1`）。

- TS 用 `**`（SLA 无该 token），此处二进制幂。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/258_pow_sqmul/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/258_pow_sqmul/main.sla
```
