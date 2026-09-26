//! opi-tools：词库编译工具（dict.yaml → .opid）。

pub mod compiler;

/// 项目宠物「小欧」的终端形象（docs/opi-pet.svg 的字符版）。
///
/// 全部选用单宽度字符：框线用 Box Drawing，眼睛用 ASCII `o`，嘴保留字母 `O`
/// （Open），刻意避开 `●` `○` `ˉ` 这类 East Asian Ambiguous 字符 —— 它们在
/// 中日韩终端里常被渲染成双宽，会让整只键帽错位。
/// 首尾各留一个空行；打印用 `print!` 而非 `println!`。
/// 注意：不要用 `"\` 续行 —— Rust 会连同下一行的前导空白一起吃掉，
/// 天线那行会左移 8 格（`pet_antenna_indent` 钉死这一点）。
pub const OPI_PET: &str = "
        -
        │
   ╭────┴────╮
   │  o   o  │
   │    O    │
╭──┴─────────┴──╮
│      OPI      │
╰───────────────╯
   ▁▁▁▁▁▁▁▁▁▁▁
";

#[cfg(test)]
mod tests {
    use super::*;

    /// 键帽四壁必须垂直对齐 —— 终端里错一格就毁容，用测试钉死。
    fn wall_columns(line: &str) -> Vec<usize> {
        line.chars()
            .enumerate()
            .filter(|(_, c)| *c == '│')
            .map(|(i, _)| i)
            .collect()
    }

    /// 天线的缩进必须原样保留（`"\` 续行会吃掉首行前导空白，导致整只键帽左移）。
    #[test]
    fn pet_antenna_indent() {
        assert!(OPI_PET.contains("\n        -\n"), "天线行缩进被吃掉：{OPI_PET:?}");
        assert!(OPI_PET.starts_with('\n') && OPI_PET.ends_with('\n'), "首尾须各留一个空行");
    }

    #[test]
    fn pet_walls_align() {
        let walls: Vec<Vec<usize>> =
            OPI_PET.lines().map(wall_columns).filter(|w| !w.is_empty()).collect();
        // 正面两行（眼睛 / 嘴巴）共用同一对墙壁。
        let face: Vec<&Vec<usize>> = walls.iter().filter(|w| w.first() == Some(&3)).collect();
        assert_eq!(face.len(), 2, "键帽正面应为两行：眼睛一行 + 嘴巴一行");
        assert!(face.iter().all(|w| w.as_slice() == [3, 13]), "正面左右壁必须对齐在 3 / 13");
        // 刻字行自成一对更宽的墙壁。
        assert!(walls.iter().any(|w| w.as_slice() == [0, 16]), "OPI 刻字行左右壁须对齐在 0 / 16");
    }
}
