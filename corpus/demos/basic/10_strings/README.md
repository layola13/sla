# 10 Strings

对标 `tsgosa/demos/10_strings` + `16_template`。

- `println("{}", s)` / `"{} world"` / `"hi {}, n={}!"`：`fmt` 占位格式化。
- `str_eq(a, b)`：字符串相等断言。无注解 `let s = "hello"` 与显式
  `let a: ptr = "hello"` 均可（见 POTENTIAL_ISSUES #3/#5，均已修复）。
- 反引号模板（`` `hi ${name}...` ``）的多插值写法目前 `check` 与 `test` 前端不一致
  且特定组合会崩溃（见 POTENTIAL_ISSUES #4），本 demo 打印统一用 `"..."` 格式化以保持 `check` 全绿。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/10_strings/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/10_strings/main.sla
```
