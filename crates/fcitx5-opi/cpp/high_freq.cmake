# 高频词库（2026-11-08）：与康熙同一条随包路径（share/opi/），fcitx5 侧首启种子
# 拷进用户词库目录一次。真源 = data/raw/high_freq.tsv（入库数据，无生成脚本）。
set(OPI_HIGH_FREQ_TSV "${CMAKE_CURRENT_SOURCE_DIR}/../../../data/raw/high_freq.tsv")
if(NOT EXISTS "${OPI_HIGH_FREQ_TSV}")
    message(FATAL_ERROR
        "找不到高频词库源: '${OPI_HIGH_FREQ_TSV}'（data/raw/high_freq.tsv 被误删？）")
endif()
message(STATUS "OPI 高频词库源: ${OPI_HIGH_FREQ_TSV}")
# 基础包也随包（65KB，属于核心候选质量）。
install(FILES "${OPI_HIGH_FREQ_TSV}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME high_freq.tsv)
