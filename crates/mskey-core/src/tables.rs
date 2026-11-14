#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tone {
    None = 0,
    Acute = 1,
    Grave = 2,
    HookAbove = 3,
    Tilde = 4,
    DotBelow = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseVowel {
    A = 0,
    ABreve = 1,
    ACircumflex = 2,
    E = 3,
    ECircumflex = 4,
    I = 5,
    O = 6,
    OCircumflex = 7,
    OHorn = 8,
    U = 9,
    UHorn = 10,
    Y = 11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VowelFamily {
    A,
    E,
    I,
    O,
    U,
    Y,
}

const VOWEL_TABLE: [[(char, char); 6]; 12] = [
    [
        ('a', 'A'),
        ('á', 'Á'),
        ('à', 'À'),
        ('ả', 'Ả'),
        ('ã', 'Ã'),
        ('ạ', 'Ạ'),
    ],
    [
        ('ă', 'Ă'),
        ('ắ', 'Ắ'),
        ('ằ', 'Ằ'),
        ('ẳ', 'Ẳ'),
        ('ẵ', 'Ẵ'),
        ('ặ', 'Ặ'),
    ],
    [
        ('â', 'Â'),
        ('ấ', 'Ấ'),
        ('ầ', 'Ầ'),
        ('ẩ', 'Ẩ'),
        ('ẫ', 'Ẫ'),
        ('ậ', 'Ậ'),
    ],
    [
        ('e', 'E'),
        ('é', 'É'),
        ('è', 'È'),
        ('ẻ', 'Ẻ'),
        ('ẽ', 'Ẽ'),
        ('ẹ', 'Ẹ'),
    ],
    [
        ('ê', 'Ê'),
        ('ế', 'Ế'),
        ('ề', 'Ề'),
        ('ể', 'Ể'),
        ('ễ', 'Ễ'),
        ('ệ', 'Ệ'),
    ],
    [
        ('i', 'I'),
        ('í', 'Í'),
        ('ì', 'Ì'),
        ('ỉ', 'Ỉ'),
        ('ĩ', 'Ĩ'),
        ('ị', 'Ị'),
    ],
    [
        ('o', 'O'),
        ('ó', 'Ó'),
        ('ò', 'Ò'),
        ('ỏ', 'Ỏ'),
        ('õ', 'Õ'),
        ('ọ', 'Ọ'),
    ],
    [
        ('ô', 'Ô'),
        ('ố', 'Ố'),
        ('ồ', 'Ồ'),
        ('ổ', 'Ổ'),
        ('ỗ', 'Ỗ'),
        ('ộ', 'Ộ'),
    ],
    [
        ('ơ', 'Ơ'),
        ('ớ', 'Ớ'),
        ('ờ', 'Ờ'),
        ('ở', 'Ở'),
        ('ỡ', 'Ỡ'),
        ('ợ', 'Ợ'),
    ],
    [
        ('u', 'U'),
        ('ú', 'Ú'),
        ('ù', 'Ù'),
        ('ủ', 'Ủ'),
        ('ũ', 'Ũ'),
        ('ụ', 'Ụ'),
    ],
    [
        ('ư', 'Ư'),
        ('ứ', 'Ứ'),
        ('ừ', 'Ừ'),
        ('ử', 'Ử'),
        ('ữ', 'Ữ'),
        ('ự', 'Ự'),
    ],
    [
        ('y', 'Y'),
        ('ý', 'Ý'),
        ('ỳ', 'Ỳ'),
        ('ỷ', 'Ỷ'),
        ('ỹ', 'Ỹ'),
        ('ỵ', 'Ỵ'),
    ],
];

const ALL_BASES: [BaseVowel; 12] = [
    BaseVowel::A,
    BaseVowel::ABreve,
    BaseVowel::ACircumflex,
    BaseVowel::E,
    BaseVowel::ECircumflex,
    BaseVowel::I,
    BaseVowel::O,
    BaseVowel::OCircumflex,
    BaseVowel::OHorn,
    BaseVowel::U,
    BaseVowel::UHorn,
    BaseVowel::Y,
];

const ALL_TONES: [Tone; 6] = [
    Tone::None,
    Tone::Acute,
    Tone::Grave,
    Tone::HookAbove,
    Tone::Tilde,
    Tone::DotBelow,
];

pub fn encode_vowel(base: BaseVowel, tone: Tone, upper: bool) -> char {
    let pair = VOWEL_TABLE[base as usize][tone as usize];
    if upper { pair.1 } else { pair.0 }
}

pub fn decode_vowel(ch: char) -> Option<(BaseVowel, Tone, bool)> {
    for (b_idx, row) in VOWEL_TABLE.iter().enumerate() {
        for (t_idx, &(lower, upper)) in row.iter().enumerate() {
            if ch == lower {
                return Some((ALL_BASES[b_idx], ALL_TONES[t_idx], false));
            }
            if ch == upper {
                return Some((ALL_BASES[b_idx], ALL_TONES[t_idx], true));
            }
        }
    }
    None
}

pub fn base_family(base: BaseVowel) -> VowelFamily {
    match base {
        BaseVowel::A | BaseVowel::ABreve | BaseVowel::ACircumflex => VowelFamily::A,
        BaseVowel::E | BaseVowel::ECircumflex => VowelFamily::E,
        BaseVowel::I => VowelFamily::I,
        BaseVowel::O | BaseVowel::OCircumflex | BaseVowel::OHorn => VowelFamily::O,
        BaseVowel::U | BaseVowel::UHorn => VowelFamily::U,
        BaseVowel::Y => VowelFamily::Y,
    }
}

pub fn strip_vowel_diacritic(base: BaseVowel) -> BaseVowel {
    match base {
        BaseVowel::A | BaseVowel::ABreve | BaseVowel::ACircumflex => BaseVowel::A,
        BaseVowel::E | BaseVowel::ECircumflex => BaseVowel::E,
        BaseVowel::I => BaseVowel::I,
        BaseVowel::O | BaseVowel::OCircumflex | BaseVowel::OHorn => BaseVowel::O,
        BaseVowel::U | BaseVowel::UHorn => BaseVowel::U,
        BaseVowel::Y => BaseVowel::Y,
    }
}

pub fn toggle_circumflex(base: BaseVowel) -> Option<(BaseVowel, bool)> {
    match base {
        BaseVowel::A => Some((BaseVowel::ACircumflex, true)),
        BaseVowel::ABreve => Some((BaseVowel::ACircumflex, true)),
        BaseVowel::ACircumflex => Some((BaseVowel::A, false)),
        BaseVowel::E => Some((BaseVowel::ECircumflex, true)),
        BaseVowel::ECircumflex => Some((BaseVowel::E, false)),
        BaseVowel::O => Some((BaseVowel::OCircumflex, true)),
        BaseVowel::OHorn => Some((BaseVowel::OCircumflex, true)),
        BaseVowel::OCircumflex => Some((BaseVowel::O, false)),
        _ => None,
    }
}

pub fn toggle_breve(base: BaseVowel) -> Option<(BaseVowel, bool)> {
    match base {
        BaseVowel::A | BaseVowel::ACircumflex => Some((BaseVowel::ABreve, true)),
        BaseVowel::ABreve => Some((BaseVowel::A, false)),
        _ => None,
    }
}

pub fn toggle_horn(base: BaseVowel) -> Option<(BaseVowel, bool)> {
    match base {
        BaseVowel::O | BaseVowel::OCircumflex => Some((BaseVowel::OHorn, true)),
        BaseVowel::OHorn => Some((BaseVowel::O, false)),
        BaseVowel::U => Some((BaseVowel::UHorn, true)),
        BaseVowel::UHorn => Some((BaseVowel::U, false)),
        _ => None,
    }
}

pub fn is_vietnamese_vowel(ch: char) -> bool {
    decode_vowel(ch).is_some()
}

pub fn is_d_or_d_stroke(ch: char) -> bool {
    matches!(ch, 'd' | 'D' | 'đ' | 'Đ')
}

pub fn to_d_stroke(upper: bool) -> char {
    if upper { 'Đ' } else { 'đ' }
}

pub fn to_plain_d(upper: bool) -> char {
    if upper { 'D' } else { 'd' }
}
