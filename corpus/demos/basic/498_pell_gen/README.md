# 498 pell gen

Pell 通解生成（`(3,2)→(17,12)→(99,70)`；488 的幂版）。

- 对标：488 pell_eq → 基本解乘幂递推（草稿函数写前即删）
- 绕行：百位打包（`y<100` 选例内安全）；`(3+2√2)` 乘法即递推

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/498_pell_gen/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/498_pell_gen/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/498_pell_gen/main.sla -o /tmp/basic_498 && /tmp/basic_498
# 期望输出：302,1712,9970
```
