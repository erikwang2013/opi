/// 全拼音节表。为保确定性排序，音节按字典序排列，查询用二分。
pub const SYLLABLES: &[&str] = &[
    "a", "ai", "an", "ang", "ao", "ba", "bai", "ban", "bang", "bao", "bei",
    "ben", "beng", "bi", "bian", "biao", "bie", "bin", "bing", "bo", "bu",
    "ca", "cai", "can", "cang", "cao", "ce", "cen", "ceng", "cha", "chai",
    "chan", "chang", "chao", "che", "chen", "cheng", "chi", "chong", "chou",
    "chu", "chua", "chuai", "chuan", "chuang", "chui", "chun", "chuo", "ci",
    "cong", "cou", "cu", "cuan", "cui", "cun", "cuo", "da", "dai", "dan",
    "dang", "dao", "de", "dei", "den", "deng", "di", "dia", "dian", "diao",
    "die", "ding", "diu", "dong", "dou", "du", "duan", "dui", "dun", "duo",
    "e", "ei", "en", "eng", "er", "fa", "fan", "fang", "fei", "fen", "feng",
    "fo", "fou", "fu", "ga", "gai", "gan", "gang", "gao", "ge", "gei", "gen",
    "geng", "gong", "gou", "gu", "gua", "guai", "guan", "guang", "gui", "gun",
    "guo", "ha", "hai", "han", "hang", "hao", "he", "hei", "hen", "heng",
    "hong", "hou", "hu", "hua", "huai", "huan", "huang", "hui", "hun", "huo",
    "ji", "jia", "jian", "jiang", "jiao", "jie", "jin", "jing", "jiong",
    "jiu", "ju", "juan", "jue", "jun", "ka", "kai", "kan", "kang", "kao",
    "ke", "kei", "ken", "keng", "kong", "kou", "ku", "kua", "kuai", "kuan",
    "kuang", "kui", "kun", "kuo", "la", "lai", "lan", "lang", "lao", "le",
    "lei", "leng", "li", "lia", "lian", "liang", "liao", "lie", "lin",
    "ling", "liu", "lo", "long", "lou", "lu", "luan", "lun", "luo", "lv",
    "lve", "ma", "mai", "man", "mang", "mao", "me", "mei", "men", "meng",
    "mi", "mian", "miao", "mie", "min", "ming", "miu", "mo", "mou", "mu",
    "na", "nai", "nan", "nang", "nao", "ne", "nei", "nen", "neng", "ni",
    "nian", "niang", "niao", "nie", "nin", "ning", "niu", "nong", "nou",
    "nu", "nuan", "nun", "nuo", "nv", "nve", "o", "ou", "pa", "pai", "pan",
    "pang", "pao", "pei", "pen", "peng", "pi", "pian", "piao", "pie", "pin",
    "ping", "po", "pou", "pu", "qi", "qia", "qian", "qiang", "qiao", "qie",
    "qin", "qing", "qiong", "qiu", "qu", "quan", "que", "qun", "ran", "rang",
    "rao", "re", "ren", "reng", "ri", "rong", "rou", "ru", "ruan", "rui",
    "run", "ruo", "sa", "sai", "san", "sang", "sao", "se", "sen", "seng",
    "sha", "shai", "shan", "shang", "shao", "she", "shei", "shen", "sheng",
    "shi", "shou", "shu", "shua", "shuai", "shuan", "shuang", "shui", "shun",
    "shuo", "si", "song", "sou", "su", "suan", "sui", "sun", "suo", "ta",
    "tai", "tan", "tang", "tao", "te", "teng", "ti", "tian", "tiao", "tie",
    "ting", "tong", "tou", "tu", "tuan", "tui", "tun", "tuo", "wa", "wai",
    "wan", "wang", "wei", "wen", "weng", "wo", "wu", "xi", "xia", "xian",
    "xiang", "xiao", "xie", "xin", "xing", "xiong", "xiu", "xu", "xuan",
    "xue", "xun", "ya", "yan", "yang", "yao", "ye", "yi", "yin", "ying",
    "yo", "yong", "you", "yu", "yuan", "yue", "yun", "za", "zai", "zan",
    "zang", "zao", "ze", "zei", "zen", "zeng", "zha", "zhai", "zhan",
    "zhang", "zhao", "zhe", "zhei", "zhen", "zheng", "zhi", "zhong", "zhou",
    "zhu", "zhua", "zhuai", "zhuan", "zhuang", "zhui", "zhun", "zhuo", "zi",
    "zong", "zou", "zu", "zuan", "zui", "zun", "zuo",
];

/// 断言音节表已按字典序排序（保护确定性）。
pub fn assert_sorted() {
    debug_assert!(SYLLABLES.windows(2).all(|w| w[0] < w[1]));
}

/// 判断 s 是否为合法音节前缀。
pub fn is_syllable_prefix(s: &str) -> bool {
    SYLLABLES
        .binary_search_by(|&cand| {
            if cand.starts_with(s) {
                std::cmp::Ordering::Equal
            } else {
                cand.as_bytes().cmp(s.as_bytes())
            }
        })
        .is_ok()
}

/// 对输入的拼音串做最长匹配切分（贪婪，最大音节长 6）。
/// `'` 为硬分隔符，单字母未匹配时按单字母切。
pub fn segment(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\'' {
            i += 1;
            continue;
        }
        let mut matched = None;
        let mut len = (chars.len() - i).min(6);
        while len >= 1 {
            let cand: String = chars[i..i + len].iter().collect();
            if is_syllable_prefix(&cand) {
                matched = Some(cand);
                break;
            }
            len -= 1;
        }
        match matched {
            Some(syl) => {
                i += syl.chars().count();
                result.push(syl);
            }
            None => {
                result.push(chars[i].to_string());
                i += 1;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syllables_sorted() {
        assert_sorted();
    }

    #[test]
    fn syllable_count_is_410() {
        assert_eq!(SYLLABLES.len(), 410);
    }

    #[test]
    fn longest_match_basic() {
        assert_eq!(segment("xian"), vec!["xian"]);
    }

    #[test]
    fn greedy_longest_chain() {
        assert_eq!(segment("shurufa"), vec!["shu", "ru", "fa"]);
    }

    #[test]
    fn apostrophe_is_hard_separator() {
        assert_eq!(segment("xi'an"), vec!["xi", "an"]);
        assert_eq!(segment("ni'hao"), vec!["ni", "hao"]);
    }

    #[test]
    fn single_letters_fallback() {
        assert_eq!(segment("abc"), vec!["a", "b", "c"]);
    }

    #[test]
    fn max_syllable_len_six() {
        assert_eq!(segment("zhuangzhuang"), vec!["zhuang", "zhuang"]);
    }

    #[test]
    fn prefix_checked() {
        assert!(is_syllable_prefix("zh"));
        assert!(!is_syllable_prefix("zx"));
    }
}
