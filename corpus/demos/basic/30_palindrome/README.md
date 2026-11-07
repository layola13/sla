# 30 Palindrome

对标 `tsgosa/demos/103_palindrome`（`abba=1, abc=0`）。

TS 用 `s.charCodeAt(i)` 双指针；SLA 的字符串索引/物化有已知坑
（POTENTIAL_ISSUES #3/#5），本 demo 用等价的 `[int; 4]` 数组回文锁定同一语义：
`[1,2,2,1]=1, [1,2,3,4]=0`。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/30_palindrome/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/30_palindrome/main.sla
```
