# 词库数据来源与许可证

| 文件 | 来源 | 许可证 | 说明 |
|---|---|---|---|
| fallback.tsv | OPI 项目自建 | MIT | 内置回退词库，由 opi-tools 编译为 data/generated/fallback.opid |
| luna_pinyin.dict.yaml（M2 验证用） | https://github.com/rime/rime-luna-pinyin | **LGPL-3.0** | 官方拼音词库，889KB ~70771 行；由 scripts/gen_luna_dict.py 重排后编译为 luna.opid（产物不直接入库）：单字第三列是读音概率（非词频），故以 GB2312 一二级码序为常用度主体、次读音排目标读音组尾、0% 读音剔除、词组按权重降序；详见脚本头注释 |
| trad_hanzi.tsv（单字） | Unicode Unihan（kMandarin 字段） | Unicode License（宽松，可再分发，保留版权声明） | GB2312 全量 6763 + 常用繁体单字，由 scripts/gen_trad_dict.py 生成（含人工常用表） |
| trad_phrases.tsv（词组） | https://github.com/rime/rime-terra-pinyin（terra_pinyin.dict.yaml） | **LGPL-3.0** | 常用繁体词组，由 scripts/gen_trad_dict.py 生成（含人工常用表） |

> 注意：rime 社区数据许可证为 LGPL-3.0（不是 BSD/GPL 混合）。使用前确认源码树内 LICENSE 文件。
