# 高频词库（2026-11-08）：与康熙同一条随包路径（share/opi/），fcitx5 侧首启种子
# 拷进用户词库目录一次。源优先级 = -D 覆盖 > crates/fcitx5-opi/data/ 的入库副本 >
# data/raw/ 真源 > android 部署副本。第一顺位是给 CI 的容器编译用的（那里只拷
# crates/fcitx5-opi 一个 crate，../../../data/raw 落到 / 上，和康熙同一条坑）。
set(OPI_HIGH_FREQ_TSV "" CACHE FILEPATH
    "high_freq.tsv 源文件。缺省取 crates/fcitx5-opi/data/ 的入库副本")
if(NOT OPI_HIGH_FREQ_TSV)
    foreach(_hf_cand
            "${CMAKE_CURRENT_SOURCE_DIR}/../data/high_freq.tsv"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../../data/raw/high_freq.tsv"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../../android/app/src/main/assets/high_freq.tsv")
        if(NOT OPI_HIGH_FREQ_TSV AND EXISTS "${_hf_cand}")
            set(OPI_HIGH_FREQ_TSV "${_hf_cand}")
        endif()
    endforeach()
endif()
if(NOT EXISTS "${OPI_HIGH_FREQ_TSV}")
    message(FATAL_ERROR
        "找不到高频词库源: '${OPI_HIGH_FREQ_TSV}'（data/raw/high_freq.tsv 被误删？）")
endif()
message(STATUS "OPI 高频词库源: ${OPI_HIGH_FREQ_TSV}")
# 基础包也随包（65KB，属于核心候选质量）。
install(FILES "${OPI_HIGH_FREQ_TSV}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME high_freq.tsv)
