# 161 Deep Break

对标 `tsgosa/demos/1053_deep_break`（`40`）。

- 32 覆盖单层 break，此处为三层 + 内外双条件；label 不支持（#9），
  内外断均用裸 break（语义与 TS 一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/161_deep_break/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/161_deep_break/main.sla
```
