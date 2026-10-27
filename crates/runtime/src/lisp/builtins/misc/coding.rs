//! Coding-system declarations, aliases and validation.

use super::err_sym;

use super::crypto::lisp_string_bytes;

use crate::lisp::eq_values;

use crate::lisp::builtins::{arg, want_int, want_list, want_string, want_sym};
use crate::lisp::{
    Interp,
    error::{EvalResult, Flow},
    obarray::sym,
    value::{StrRef, Value},
};

// ---------- coding systems ----------

/// Coding systems defined by GNU Emacs batch startup.
pub(crate) const CODING_SYSTEMS: &[&str] = &[
    "binary",
    "no-conversion",
    "undecided",
    "prefer-utf-8",
    "raw-text",
    "no-conversion-multibyte",
    "latin-1",
    "iso-8859-1",
    "iso-latin-1",
    "emacs-mule",
    "cp65001",
    "mule-utf-8",
    "utf-8",
    "utf-8-with-signature",
    "utf-8-auto",
    "utf-8-emacs",
    "utf-16le",
    "utf-16be",
    "utf-16-le",
    "utf-16le-with-signature",
    "utf-16-be",
    "utf-16be-with-signature",
    "utf-16",
    "iso-2022-7bit",
    "iso-2022-7bit-ss2",
    "iso-2022-int-1",
    "iso-2022-7bit-lock",
    "iso-2022-cjk",
    "iso-2022-7bit-lock-ss2",
    "iso-2022-8bit-ss2",
    "ctext",
    "x-ctext",
    "compound-text",
    "ctext-no-compositions",
    "ctext-with-extensions",
    "x-ctext-with-extensions",
    "compound-text-with-extensions",
    "ascii",
    "iso-safe",
    "us-ascii",
    "utf-7",
    "utf-7-imap",
    "chinese-iso-7bit",
    "iso-2022-cn",
    "iso-2022-cn-ext",
    "gb2312",
    "cn-gb",
    "euc-cn",
    "euc-china",
    "cn-gb-2312",
    "chinese-iso-8bit",
    "hz",
    "hz-gb-2312",
    "chinese-hz",
    "cp950",
    "cn-big5",
    "big5",
    "chinese-big5",
    "cn-big5-hkscs",
    "big5-hkscs",
    "chinese-big5-hkscs",
    "euc-taiwan",
    "euc-tw",
    "windows-936",
    "cp936",
    "gbk",
    "chinese-gbk",
    "gb18030",
    "chinese-gb18030",
    "iso-8859-5",
    "cyrillic-iso-8bit",
    "cp878",
    "koi8",
    "koi8-r",
    "cyrillic-koi8",
    "koi8-u",
    "alternativnyj",
    "cyrillic-alternativnyj",
    "cp866",
    "koi8-t",
    "cp1251",
    "windows-1251",
    "cp866u",
    "ruscii",
    "cp1125",
    "ibm855",
    "cp855",
    "mik",
    "pt154",
    "devanagari",
    "in-is13194-devanagari",
    "ebcdic-us",
    "ebcdic-uk",
    "cp1047",
    "ibm1047",
    "cp038",
    "ebcdic-int",
    "ibm038",
    "latin-2",
    "iso-8859-2",
    "iso-latin-2",
    "latin-3",
    "iso-8859-3",
    "iso-latin-3",
    "latin-4",
    "iso-8859-4",
    "iso-latin-4",
    "latin-5",
    "iso-8859-9",
    "iso-latin-5",
    "latin-6",
    "iso-8859-10",
    "iso-latin-6",
    "latin-7",
    "iso-8859-13",
    "iso-latin-7",
    "latin-8",
    "iso-8859-14",
    "iso-latin-8",
    "latin-0",
    "latin-9",
    "iso-8859-15",
    "iso-latin-9",
    "cp1250",
    "windows-1250",
    "cp1252",
    "windows-1252",
    "cp1254",
    "windows-1254",
    "cp1257",
    "windows-1257",
    "cp256",
    "ebcdic-int1",
    "ibm256",
    "cp273",
    "ibm273",
    "cp274",
    "ebcdic-be",
    "ibm274",
    "cp275",
    "ebcdic-br",
    "ibm275",
    "cp277",
    "ebcdic-cp-no",
    "ebcdic-cp-dk",
    "ibm277",
    "cp278",
    "ebcdic-cp-se",
    "ebcdic-cp-fi",
    "ibm278",
    "cp280",
    "ebcdic-cp-it",
    "ibm280",
    "cp284",
    "ebcdic-cp-es",
    "ibm284",
    "cp285",
    "ebcdic-cp-gb",
    "ibm285",
    "cp297",
    "ebcdic-cp-fr",
    "ibm297",
    "ibm775",
    "cp775",
    "ibm850",
    "cp850",
    "ibm852",
    "cp852",
    "ibm857",
    "cp857",
    "cp858",
    "ibm860",
    "cp860",
    "ibm861",
    "cp861",
    "ibm863",
    "cp863",
    "ibm865",
    "cp865",
    "ibm437",
    "cp437",
    "macintosh",
    "mac-roman",
    "next",
    "roman8",
    "hp-roman8",
    "adobe-standard-encoding",
    "latin-10",
    "iso-8859-16",
    "iso-latin-10",
    "iso-8859-7",
    "greek-iso-8bit",
    "cp1253",
    "windows-1253",
    "cp737",
    "ibm851",
    "cp851",
    "ibm869",
    "cp869",
    "iso-8859-8-i",
    "iso-8859-8-e",
    "iso-8859-8",
    "hebrew-iso-8bit",
    "cp1255",
    "windows-1255",
    "ibm862",
    "cp862",
    "junet",
    "iso-2022-jp",
    "iso-2022-jp-2",
    "sjis",
    "shift_jis",
    "japanese-shift-jis",
    "cp932",
    "japanese-cp932",
    "old-jis",
    "iso-2022-jp-1978-irv",
    "japanese-iso-7bit-1978-irv",
    "euc-jp",
    "euc-japan",
    "euc-japan-1990",
    "japanese-iso-8bit",
    "eucjp-ms",
    "iso-2022-jp-3",
    "iso-2022-jp-2004",
    "euc-jisx0213",
    "euc-jis-2004",
    "shift_jis-2004",
    "japanese-shift-jis-2004",
    "cp281",
    "ebcdic-jp-e",
    "ibm281",
    "cp290",
    "ebcdic-jp-kana",
    "ibm290",
    "ks_c_5601-1987",
    "euc-korea",
    "euc-kr",
    "korean-iso-8bit",
    "korean-iso-7bit-lock",
    "iso-2022-kr",
    "cp949",
    "korean-cp949",
    "lao",
    "tis-620",
    "tis620",
    "th-tis620",
    "thai-tis620",
    "ibm874",
    "cp874",
    "iso-8859-11",
    "tibetan",
    "tibetan-iso-8bit",
    "viscii",
    "vietnamese-viscii",
    "tcvn-5712",
    "tcvn",
    "vietnamese-tcvn",
    "vscii",
    "vietnamese-vscii",
    "viqr",
    "vietnamese-viqr",
    "cp1258",
    "windows-1258",
    "iso-8859-6",
    "cp1256",
    "windows-1256",
    "georgian-ps",
    "georgian-academy",
    "utf-8-nfd",
    "utf-8-hfs",
];

pub(crate) const CODING_ALIASES: &[(&str, &[&str])] = &[
    ("binary", &["no-conversion", "binary"]),
    ("no-conversion", &["no-conversion", "binary"]),
    ("undecided", &["undecided"]),
    ("prefer-utf-8", &["prefer-utf-8"]),
    ("raw-text", &["raw-text"]),
    ("no-conversion-multibyte", &["no-conversion-multibyte"]),
    ("latin-1", &["iso-latin-1", "iso-8859-1", "latin-1"]),
    ("iso-8859-1", &["iso-latin-1", "iso-8859-1", "latin-1"]),
    ("iso-latin-1", &["iso-latin-1", "iso-8859-1", "latin-1"]),
    ("emacs-mule", &["emacs-mule"]),
    ("cp65001", &["utf-8", "mule-utf-8", "cp65001"]),
    ("mule-utf-8", &["utf-8", "mule-utf-8", "cp65001"]),
    ("utf-8", &["utf-8", "mule-utf-8", "cp65001"]),
    ("utf-8-with-signature", &["utf-8-with-signature"]),
    ("utf-8-auto", &["utf-8-auto"]),
    ("utf-8-emacs", &["utf-8-emacs"]),
    ("utf-16le", &["utf-16le"]),
    ("utf-16be", &["utf-16be"]),
    ("utf-16-le", &["utf-16le-with-signature", "utf-16-le"]),
    (
        "utf-16le-with-signature",
        &["utf-16le-with-signature", "utf-16-le"],
    ),
    ("utf-16-be", &["utf-16be-with-signature", "utf-16-be"]),
    (
        "utf-16be-with-signature",
        &["utf-16be-with-signature", "utf-16-be"],
    ),
    ("utf-16", &["utf-16"]),
    ("iso-2022-7bit", &["iso-2022-7bit"]),
    ("iso-2022-7bit-ss2", &["iso-2022-7bit-ss2"]),
    ("iso-2022-int-1", &["iso-2022-7bit-lock", "iso-2022-int-1"]),
    (
        "iso-2022-7bit-lock",
        &["iso-2022-7bit-lock", "iso-2022-int-1"],
    ),
    ("iso-2022-cjk", &["iso-2022-7bit-lock-ss2", "iso-2022-cjk"]),
    (
        "iso-2022-7bit-lock-ss2",
        &["iso-2022-7bit-lock-ss2", "iso-2022-cjk"],
    ),
    ("iso-2022-8bit-ss2", &["iso-2022-8bit-ss2"]),
    ("ctext", &["compound-text", "x-ctext", "ctext"]),
    ("x-ctext", &["compound-text", "x-ctext", "ctext"]),
    ("compound-text", &["compound-text", "x-ctext", "ctext"]),
    ("ctext-no-compositions", &["ctext-no-compositions"]),
    (
        "ctext-with-extensions",
        &[
            "compound-text-with-extensions",
            "x-ctext-with-extensions",
            "ctext-with-extensions",
        ],
    ),
    (
        "x-ctext-with-extensions",
        &[
            "compound-text-with-extensions",
            "x-ctext-with-extensions",
            "ctext-with-extensions",
        ],
    ),
    (
        "compound-text-with-extensions",
        &[
            "compound-text-with-extensions",
            "x-ctext-with-extensions",
            "ctext-with-extensions",
        ],
    ),
    ("ascii", &["us-ascii", "iso-safe", "ascii"]),
    ("iso-safe", &["us-ascii", "iso-safe", "ascii"]),
    ("us-ascii", &["us-ascii", "iso-safe", "ascii"]),
    ("utf-7", &["utf-7"]),
    ("utf-7-imap", &["utf-7-imap"]),
    ("chinese-iso-7bit", &["iso-2022-cn", "chinese-iso-7bit"]),
    ("iso-2022-cn", &["iso-2022-cn", "chinese-iso-7bit"]),
    ("iso-2022-cn-ext", &["iso-2022-cn-ext"]),
    (
        "gb2312",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "cn-gb",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "euc-cn",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "euc-china",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "cn-gb-2312",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    (
        "chinese-iso-8bit",
        &[
            "chinese-iso-8bit",
            "cn-gb-2312",
            "euc-china",
            "euc-cn",
            "cn-gb",
            "gb2312",
        ],
    ),
    ("hz", &["chinese-hz", "hz-gb-2312", "hz"]),
    ("hz-gb-2312", &["chinese-hz", "hz-gb-2312", "hz"]),
    ("chinese-hz", &["chinese-hz", "hz-gb-2312", "hz"]),
    ("cp950", &["chinese-big5", "big5", "cn-big5", "cp950"]),
    ("cn-big5", &["chinese-big5", "big5", "cn-big5", "cp950"]),
    ("big5", &["chinese-big5", "big5", "cn-big5", "cp950"]),
    (
        "chinese-big5",
        &["chinese-big5", "big5", "cn-big5", "cp950"],
    ),
    (
        "cn-big5-hkscs",
        &["chinese-big5-hkscs", "big5-hkscs", "cn-big5-hkscs"],
    ),
    (
        "big5-hkscs",
        &["chinese-big5-hkscs", "big5-hkscs", "cn-big5-hkscs"],
    ),
    (
        "chinese-big5-hkscs",
        &["chinese-big5-hkscs", "big5-hkscs", "cn-big5-hkscs"],
    ),
    ("euc-taiwan", &["euc-tw", "euc-taiwan"]),
    ("euc-tw", &["euc-tw", "euc-taiwan"]),
    (
        "windows-936",
        &["chinese-gbk", "gbk", "cp936", "windows-936"],
    ),
    ("cp936", &["chinese-gbk", "gbk", "cp936", "windows-936"]),
    ("gbk", &["chinese-gbk", "gbk", "cp936", "windows-936"]),
    (
        "chinese-gbk",
        &["chinese-gbk", "gbk", "cp936", "windows-936"],
    ),
    ("gb18030", &["chinese-gb18030", "gb18030"]),
    ("chinese-gb18030", &["chinese-gb18030", "gb18030"]),
    ("iso-8859-5", &["cyrillic-iso-8bit", "iso-8859-5"]),
    ("cyrillic-iso-8bit", &["cyrillic-iso-8bit", "iso-8859-5"]),
    ("cp878", &["cyrillic-koi8", "koi8-r", "koi8", "cp878"]),
    ("koi8", &["cyrillic-koi8", "koi8-r", "koi8", "cp878"]),
    ("koi8-r", &["cyrillic-koi8", "koi8-r", "koi8", "cp878"]),
    (
        "cyrillic-koi8",
        &["cyrillic-koi8", "koi8-r", "koi8", "cp878"],
    ),
    ("koi8-u", &["koi8-u"]),
    (
        "alternativnyj",
        &["cyrillic-alternativnyj", "alternativnyj"],
    ),
    (
        "cyrillic-alternativnyj",
        &["cyrillic-alternativnyj", "alternativnyj"],
    ),
    ("cp866", &["cp866"]),
    ("koi8-t", &["koi8-t"]),
    ("cp1251", &["windows-1251", "cp1251"]),
    ("windows-1251", &["windows-1251", "cp1251"]),
    ("cp866u", &["cp1125", "ruscii", "cp866u"]),
    ("ruscii", &["cp1125", "ruscii", "cp866u"]),
    ("cp1125", &["cp1125", "ruscii", "cp866u"]),
    ("ibm855", &["cp855", "ibm855"]),
    ("cp855", &["cp855", "ibm855"]),
    ("mik", &["mik"]),
    ("pt154", &["pt154"]),
    ("devanagari", &["in-is13194-devanagari", "devanagari"]),
    (
        "in-is13194-devanagari",
        &["in-is13194-devanagari", "devanagari"],
    ),
    ("ebcdic-us", &["ebcdic-us"]),
    ("ebcdic-uk", &["ebcdic-uk"]),
    ("cp1047", &["ibm1047", "cp1047"]),
    ("ibm1047", &["ibm1047", "cp1047"]),
    ("cp038", &["ibm038", "ebcdic-int", "cp038"]),
    ("ebcdic-int", &["ibm038", "ebcdic-int", "cp038"]),
    ("ibm038", &["ibm038", "ebcdic-int", "cp038"]),
    ("latin-2", &["iso-latin-2", "iso-8859-2", "latin-2"]),
    ("iso-8859-2", &["iso-latin-2", "iso-8859-2", "latin-2"]),
    ("iso-latin-2", &["iso-latin-2", "iso-8859-2", "latin-2"]),
    ("latin-3", &["iso-latin-3", "iso-8859-3", "latin-3"]),
    ("iso-8859-3", &["iso-latin-3", "iso-8859-3", "latin-3"]),
    ("iso-latin-3", &["iso-latin-3", "iso-8859-3", "latin-3"]),
    ("latin-4", &["iso-latin-4", "iso-8859-4", "latin-4"]),
    ("iso-8859-4", &["iso-latin-4", "iso-8859-4", "latin-4"]),
    ("iso-latin-4", &["iso-latin-4", "iso-8859-4", "latin-4"]),
    ("latin-5", &["iso-latin-5", "iso-8859-9", "latin-5"]),
    ("iso-8859-9", &["iso-latin-5", "iso-8859-9", "latin-5"]),
    ("iso-latin-5", &["iso-latin-5", "iso-8859-9", "latin-5"]),
    ("latin-6", &["iso-latin-6", "iso-8859-10", "latin-6"]),
    ("iso-8859-10", &["iso-latin-6", "iso-8859-10", "latin-6"]),
    ("iso-latin-6", &["iso-latin-6", "iso-8859-10", "latin-6"]),
    ("latin-7", &["iso-latin-7", "iso-8859-13", "latin-7"]),
    ("iso-8859-13", &["iso-latin-7", "iso-8859-13", "latin-7"]),
    ("iso-latin-7", &["iso-latin-7", "iso-8859-13", "latin-7"]),
    ("latin-8", &["iso-latin-8", "iso-8859-14", "latin-8"]),
    ("iso-8859-14", &["iso-latin-8", "iso-8859-14", "latin-8"]),
    ("iso-latin-8", &["iso-latin-8", "iso-8859-14", "latin-8"]),
    (
        "latin-0",
        &["iso-latin-9", "iso-8859-15", "latin-9", "latin-0"],
    ),
    (
        "latin-9",
        &["iso-latin-9", "iso-8859-15", "latin-9", "latin-0"],
    ),
    (
        "iso-8859-15",
        &["iso-latin-9", "iso-8859-15", "latin-9", "latin-0"],
    ),
    (
        "iso-latin-9",
        &["iso-latin-9", "iso-8859-15", "latin-9", "latin-0"],
    ),
    ("cp1250", &["windows-1250", "cp1250"]),
    ("windows-1250", &["windows-1250", "cp1250"]),
    ("cp1252", &["windows-1252", "cp1252"]),
    ("windows-1252", &["windows-1252", "cp1252"]),
    ("cp1254", &["windows-1254", "cp1254"]),
    ("windows-1254", &["windows-1254", "cp1254"]),
    ("cp1257", &["windows-1257", "cp1257"]),
    ("windows-1257", &["windows-1257", "cp1257"]),
    ("cp256", &["ibm256", "ebcdic-int1", "cp256"]),
    ("ebcdic-int1", &["ibm256", "ebcdic-int1", "cp256"]),
    ("ibm256", &["ibm256", "ebcdic-int1", "cp256"]),
    ("cp273", &["ibm273", "cp273"]),
    ("ibm273", &["ibm273", "cp273"]),
    ("cp274", &["ibm274", "ebcdic-be", "cp274"]),
    ("ebcdic-be", &["ibm274", "ebcdic-be", "cp274"]),
    ("ibm274", &["ibm274", "ebcdic-be", "cp274"]),
    ("cp275", &["ibm275", "ebcdic-br", "cp275"]),
    ("ebcdic-br", &["ibm275", "ebcdic-br", "cp275"]),
    ("ibm275", &["ibm275", "ebcdic-br", "cp275"]),
    (
        "cp277",
        &["ibm277", "ebcdic-cp-dk", "ebcdic-cp-no", "cp277"],
    ),
    (
        "ebcdic-cp-no",
        &["ibm277", "ebcdic-cp-dk", "ebcdic-cp-no", "cp277"],
    ),
    (
        "ebcdic-cp-dk",
        &["ibm277", "ebcdic-cp-dk", "ebcdic-cp-no", "cp277"],
    ),
    (
        "ibm277",
        &["ibm277", "ebcdic-cp-dk", "ebcdic-cp-no", "cp277"],
    ),
    (
        "cp278",
        &["ibm278", "ebcdic-cp-fi", "ebcdic-cp-se", "cp278"],
    ),
    (
        "ebcdic-cp-se",
        &["ibm278", "ebcdic-cp-fi", "ebcdic-cp-se", "cp278"],
    ),
    (
        "ebcdic-cp-fi",
        &["ibm278", "ebcdic-cp-fi", "ebcdic-cp-se", "cp278"],
    ),
    (
        "ibm278",
        &["ibm278", "ebcdic-cp-fi", "ebcdic-cp-se", "cp278"],
    ),
    ("cp280", &["ibm280", "ebcdic-cp-it", "cp280"]),
    ("ebcdic-cp-it", &["ibm280", "ebcdic-cp-it", "cp280"]),
    ("ibm280", &["ibm280", "ebcdic-cp-it", "cp280"]),
    ("cp284", &["ibm284", "ebcdic-cp-es", "cp284"]),
    ("ebcdic-cp-es", &["ibm284", "ebcdic-cp-es", "cp284"]),
    ("ibm284", &["ibm284", "ebcdic-cp-es", "cp284"]),
    ("cp285", &["ibm285", "ebcdic-cp-gb", "cp285"]),
    ("ebcdic-cp-gb", &["ibm285", "ebcdic-cp-gb", "cp285"]),
    ("ibm285", &["ibm285", "ebcdic-cp-gb", "cp285"]),
    ("cp297", &["ibm297", "ebcdic-cp-fr", "cp297"]),
    ("ebcdic-cp-fr", &["ibm297", "ebcdic-cp-fr", "cp297"]),
    ("ibm297", &["ibm297", "ebcdic-cp-fr", "cp297"]),
    ("ibm775", &["cp775", "ibm775"]),
    ("cp775", &["cp775", "ibm775"]),
    ("ibm850", &["cp850", "ibm850"]),
    ("cp850", &["cp850", "ibm850"]),
    ("ibm852", &["cp852", "ibm852"]),
    ("cp852", &["cp852", "ibm852"]),
    ("ibm857", &["cp857", "ibm857"]),
    ("cp857", &["cp857", "ibm857"]),
    ("cp858", &["cp858"]),
    ("ibm860", &["cp860", "ibm860"]),
    ("cp860", &["cp860", "ibm860"]),
    ("ibm861", &["cp861", "ibm861"]),
    ("cp861", &["cp861", "ibm861"]),
    ("ibm863", &["cp863", "ibm863"]),
    ("cp863", &["cp863", "ibm863"]),
    ("ibm865", &["cp865", "ibm865"]),
    ("cp865", &["cp865", "ibm865"]),
    ("ibm437", &["cp437", "ibm437"]),
    ("cp437", &["cp437", "ibm437"]),
    ("macintosh", &["mac-roman", "macintosh"]),
    ("mac-roman", &["mac-roman", "macintosh"]),
    ("next", &["next"]),
    ("roman8", &["hp-roman8", "roman8"]),
    ("hp-roman8", &["hp-roman8", "roman8"]),
    ("adobe-standard-encoding", &["adobe-standard-encoding"]),
    ("latin-10", &["iso-latin-10", "iso-8859-16", "latin-10"]),
    ("iso-8859-16", &["iso-latin-10", "iso-8859-16", "latin-10"]),
    ("iso-latin-10", &["iso-latin-10", "iso-8859-16", "latin-10"]),
    ("iso-8859-7", &["greek-iso-8bit", "iso-8859-7"]),
    ("greek-iso-8bit", &["greek-iso-8bit", "iso-8859-7"]),
    ("cp1253", &["windows-1253", "cp1253"]),
    ("windows-1253", &["windows-1253", "cp1253"]),
    ("cp737", &["cp737"]),
    ("ibm851", &["cp851", "ibm851"]),
    ("cp851", &["cp851", "ibm851"]),
    ("ibm869", &["cp869", "ibm869"]),
    ("cp869", &["cp869", "ibm869"]),
    (
        "iso-8859-8-i",
        &[
            "hebrew-iso-8bit",
            "iso-8859-8",
            "iso-8859-8-e",
            "iso-8859-8-i",
        ],
    ),
    (
        "iso-8859-8-e",
        &[
            "hebrew-iso-8bit",
            "iso-8859-8",
            "iso-8859-8-e",
            "iso-8859-8-i",
        ],
    ),
    (
        "iso-8859-8",
        &[
            "hebrew-iso-8bit",
            "iso-8859-8",
            "iso-8859-8-e",
            "iso-8859-8-i",
        ],
    ),
    (
        "hebrew-iso-8bit",
        &[
            "hebrew-iso-8bit",
            "iso-8859-8",
            "iso-8859-8-e",
            "iso-8859-8-i",
        ],
    ),
    ("cp1255", &["windows-1255", "cp1255"]),
    ("windows-1255", &["windows-1255", "cp1255"]),
    ("ibm862", &["cp862", "ibm862"]),
    ("cp862", &["cp862", "ibm862"]),
    ("junet", &["iso-2022-jp", "junet"]),
    ("iso-2022-jp", &["iso-2022-jp", "junet"]),
    ("iso-2022-jp-2", &["iso-2022-jp-2"]),
    ("sjis", &["japanese-shift-jis", "shift_jis", "sjis"]),
    ("shift_jis", &["japanese-shift-jis", "shift_jis", "sjis"]),
    (
        "japanese-shift-jis",
        &["japanese-shift-jis", "shift_jis", "sjis"],
    ),
    ("cp932", &["japanese-cp932", "cp932"]),
    ("japanese-cp932", &["japanese-cp932", "cp932"]),
    (
        "old-jis",
        &[
            "japanese-iso-7bit-1978-irv",
            "iso-2022-jp-1978-irv",
            "old-jis",
        ],
    ),
    (
        "iso-2022-jp-1978-irv",
        &[
            "japanese-iso-7bit-1978-irv",
            "iso-2022-jp-1978-irv",
            "old-jis",
        ],
    ),
    (
        "japanese-iso-7bit-1978-irv",
        &[
            "japanese-iso-7bit-1978-irv",
            "iso-2022-jp-1978-irv",
            "old-jis",
        ],
    ),
    (
        "euc-jp",
        &["japanese-iso-8bit", "euc-japan-1990", "euc-japan", "euc-jp"],
    ),
    (
        "euc-japan",
        &["japanese-iso-8bit", "euc-japan-1990", "euc-japan", "euc-jp"],
    ),
    (
        "euc-japan-1990",
        &["japanese-iso-8bit", "euc-japan-1990", "euc-japan", "euc-jp"],
    ),
    (
        "japanese-iso-8bit",
        &["japanese-iso-8bit", "euc-japan-1990", "euc-japan", "euc-jp"],
    ),
    ("eucjp-ms", &["eucjp-ms"]),
    ("iso-2022-jp-3", &["iso-2022-jp-2004", "iso-2022-jp-3"]),
    ("iso-2022-jp-2004", &["iso-2022-jp-2004", "iso-2022-jp-3"]),
    ("euc-jisx0213", &["euc-jis-2004", "euc-jisx0213"]),
    ("euc-jis-2004", &["euc-jis-2004", "euc-jisx0213"]),
    (
        "shift_jis-2004",
        &["japanese-shift-jis-2004", "shift_jis-2004"],
    ),
    (
        "japanese-shift-jis-2004",
        &["japanese-shift-jis-2004", "shift_jis-2004"],
    ),
    ("cp281", &["ibm281", "ebcdic-jp-e", "cp281"]),
    ("ebcdic-jp-e", &["ibm281", "ebcdic-jp-e", "cp281"]),
    ("ibm281", &["ibm281", "ebcdic-jp-e", "cp281"]),
    ("cp290", &["ibm290", "ebcdic-jp-kana", "cp290"]),
    ("ebcdic-jp-kana", &["ibm290", "ebcdic-jp-kana", "cp290"]),
    ("ibm290", &["ibm290", "ebcdic-jp-kana", "cp290"]),
    (
        "ks_c_5601-1987",
        &["korean-iso-8bit", "euc-kr", "euc-korea", "ks_c_5601-1987"],
    ),
    (
        "euc-korea",
        &["korean-iso-8bit", "euc-kr", "euc-korea", "ks_c_5601-1987"],
    ),
    (
        "euc-kr",
        &["korean-iso-8bit", "euc-kr", "euc-korea", "ks_c_5601-1987"],
    ),
    (
        "korean-iso-8bit",
        &["korean-iso-8bit", "euc-kr", "euc-korea", "ks_c_5601-1987"],
    ),
    (
        "korean-iso-7bit-lock",
        &["iso-2022-kr", "korean-iso-7bit-lock"],
    ),
    ("iso-2022-kr", &["iso-2022-kr", "korean-iso-7bit-lock"]),
    ("cp949", &["korean-cp949", "cp949"]),
    ("korean-cp949", &["korean-cp949", "cp949"]),
    ("lao", &["lao"]),
    (
        "tis-620",
        &["thai-tis620", "th-tis620", "tis620", "tis-620"],
    ),
    ("tis620", &["thai-tis620", "th-tis620", "tis620", "tis-620"]),
    (
        "th-tis620",
        &["thai-tis620", "th-tis620", "tis620", "tis-620"],
    ),
    (
        "thai-tis620",
        &["thai-tis620", "th-tis620", "tis620", "tis-620"],
    ),
    ("ibm874", &["cp874", "ibm874"]),
    ("cp874", &["cp874", "ibm874"]),
    ("iso-8859-11", &["iso-8859-11"]),
    ("tibetan", &["tibetan-iso-8bit", "tibetan"]),
    ("tibetan-iso-8bit", &["tibetan-iso-8bit", "tibetan"]),
    ("viscii", &["vietnamese-viscii", "viscii"]),
    ("vietnamese-viscii", &["vietnamese-viscii", "viscii"]),
    (
        "tcvn-5712",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    (
        "tcvn",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    (
        "vietnamese-tcvn",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    (
        "vscii",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    (
        "vietnamese-vscii",
        &[
            "vietnamese-vscii",
            "vscii",
            "vietnamese-tcvn",
            "tcvn",
            "tcvn-5712",
        ],
    ),
    ("viqr", &["vietnamese-viqr", "viqr"]),
    ("vietnamese-viqr", &["vietnamese-viqr", "viqr"]),
    ("cp1258", &["windows-1258", "cp1258"]),
    ("windows-1258", &["windows-1258", "cp1258"]),
    ("iso-8859-6", &["iso-8859-6"]),
    ("cp1256", &["windows-1256", "cp1256"]),
    ("windows-1256", &["windows-1256", "cp1256"]),
    ("georgian-ps", &["georgian-ps"]),
    ("georgian-academy", &["georgian-academy"]),
    ("utf-8-nfd", &["utf-8-hfs", "utf-8-nfd"]),
    ("utf-8-hfs", &["utf-8-hfs", "utf-8-nfd"]),
    // GNU: `define-coding-system-alias 'emacs-internal 'utf-8-emacs-unix'
    // — a pure alias, so absent from `coding-system-list' but known to
    // `coding-system-p'.
    ("emacs-internal", &["utf-8-emacs-unix", "emacs-internal"]),
];

pub(crate) fn coding_known(i: &Interp, v: &Value) -> Option<String> {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return None,
    };
    // Registered subsidiaries remain valid even if a parent alias is
    // later redirected to a coding system with a fixed EOL type.
    if crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED.contains(&name.as_str()) {
        return Some(name);
    }
    // Explicit alias names are arbitrary symbols, even when their spelling
    // ends in an EOL suffix whose apparent base is not a coding system.
    if i.extra_coding_aliases
        .iter()
        .any(|(alias, _)| *alias == name)
    {
        return Some(name);
    }
    let base = name
        .strip_suffix("-unix")
        .or_else(|| name.strip_suffix("-dos"))
        .or_else(|| name.strip_suffix("-mac"))
        .unwrap_or(&name);
    // Only automatic-EOL parents own subsidiaries. Aliases of a fixed
    // subsidiary (emacs-internal -> utf-8-emacs-unix) do not gain them.
    let mut parent = base.to_string();
    for _ in 0..32 {
        if let Some((_, target)) = i.extra_coding_aliases.iter().find(|(n, _)| *n == parent) {
            parent = target.clone();
        } else {
            break;
        }
    }
    if let Some((_, group)) = CODING_ALIASES.iter().find(|(n, _)| *n == parent) {
        parent = group[0].to_string();
    }
    let eol_ok = base == name.as_str()
        || (eol_split(&parent).1.is_none()
            && !matches!(parent.as_str(), "no-conversion" | "unix" | "dos" | "mac")
            && (CODING_SYSTEMS.contains(&base)
                || crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED.contains(&base)
                || i.extra_coding_systems.iter().any(|n| *n == base)));
    if eol_ok
        && (CODING_SYSTEMS.contains(&name.as_str())
            || CODING_SYSTEMS.contains(&base)
            || crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED.contains(&name.as_str())
            || crate::lisp::builtins::csl_table::CODING_SYSTEM_LIST_SEED.contains(&base)
            || CODING_ALIASES.iter().any(|&(n, _)| n == name)
            || i.extra_coding_systems.iter().any(|n| *n == name)
            || i.extra_coding_aliases.iter().any(|(n, _)| *n == name))
    {
        Some(name)
    } else {
        None
    }
}

/// GNU Fcheck_coding_system's two-stage check used by every
/// `coding-system-*` subr: a non-symbol argument signals
/// `wrong-type-argument symbolp'; a symbol naming no coding system
/// signals `coding-system-error'.  Returns the canonical name.
pub(crate) fn check_coding(i: &mut Interp, v: &Value) -> Result<String, Flow> {
    match v {
        Value::Sym(_) => match coding_known(i, v) {
            Some(n) => Ok(n),
            None => {
                let cs = i.q("coding-system-error");
                Err(i.signal_data(cs, vec![v.clone()]))
            }
        },
        other => Err(i.wrong_type_mut("symbolp", other)),
    }
}

pub(super) fn f_coding_system_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU (mule.el): iterate the `coding-system-list' VARIABLE, keep
    // members that are their own base, and (unless BASE-ONLY) expand
    // each through `coding-system-aliases'; `cons'ing accumulates the
    // reverse of the variable order.  The Lisp defun shadows this subr
    // once mule.el loads — keep both implementations consistent.
    let base_only = arg(&a, 0).truthy();
    let csl = i.q("coding-system-list");
    let members = i.symbol_value(csl).list_to_vec().unwrap_or_default();
    let mut codings: Vec<Value> = Vec::new();
    for member in members {
        let Value::Sym(_) = member else { continue };
        let base = f_coding_system_base(i, vec![member.clone()].into())?;
        if !eq_values(&base, &member) {
            continue;
        }
        if base_only {
            codings.push(member);
            continue;
        }
        match coding_alias_group(i, &member) {
            Some(group) if !group.is_empty() => {
                for n in group {
                    codings.push(Value::Sym(i.intern(&n)));
                }
            }
            _ => codings.push(member),
        }
    }
    codings.reverse();
    Ok(Value::list(codings))
}
pub(super) fn f_coding_system_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(
        a[0].is_nil() || coding_known(i, &a[0]).is_some(),
    ))
}

pub(super) fn f_coding_system_priority_list(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU builds this from the per-category priority table; we keep
    // the same list in `remacs-coding-system-priorities', seeded to
    // GNU's -Q order and updated by `set-coding-system-priority'.
    let var = i.q("remacs-coding-system-priorities");
    let list = i.symbol_value(var);
    if arg(&a, 0).truthy() {
        if let Value::Cons(c) = &list {
            return Ok(c.borrow().car.clone());
        }
    }
    Ok(list)
}

pub(super) fn f_check_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil is accepted (returned as-is), non-symbols signal
    // `wrong-type-argument symbolp', unknown symbols signal
    // `coding-system-error' (verified against 31.1).
    match &a[0] {
        Value::Nil => return Ok(Value::Nil),
        Value::Sym(_) => {}
        v => return Err(i.wrong_type_mut("symbolp", v)),
    }
    match coding_known(i, &a[0]) {
        Some(_) => Ok(a[0].clone()),
        None => {
            let s = i.q("coding-system-error");
            Err(i.signal_data(s, vec![a[0].clone()]))
        }
    }
}

/// Strip a `-unix'/`-dos'/`-mac' EOL suffix, returning the base name
/// and variant index (0/1/2); None when there's no suffix.
pub(super) fn eol_split(name: &str) -> (&str, Option<usize>) {
    for (suf, idx) in [("-unix", 0usize), ("-dos", 1), ("-mac", 2)] {
        if let Some(b) = name.strip_suffix(suf) {
            return (b, Some(idx));
        }
    }
    (name, None)
}

/// The alias group (canonical name first) for a known coding-system
/// name, resolving EOL variants to their base first; runtime aliases
/// defined via `define-coding-system-alias' join their target's group
/// appended (GNU extends the group's alias list).
pub(crate) fn coding_alias_group(i: &Interp, v: &Value) -> Option<Vec<String>> {
    let name = match v {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return None,
    };
    // GNU's model: every coding system X owns its `aliases' attribute
    // — a list whose head is X itself.  `define-coding-system-alias'
    // appends the alias to the *resolved* system's list, so `unix'
    // (→ undecided-unix) lands in `undecided-unix's group, never in
    // `undecided's.
    let resolve_target = |i: &Interp, n: &str| -> String {
        let mut cur = n.to_string();
        for _ in 0..16 {
            match i.extra_coding_aliases.iter().find(|(x, _)| *x == cur) {
                Some((_, t)) => cur = t.clone(),
                None => break,
            }
        }
        cur
    };
    // The canonical system NAME resolves to (base-group member or
    // custom system), plus the group that head owns.
    let name = resolve_target(i, &name);
    let head = {
        let (nb, idx) = eol_split(&name);
        let base_res = resolve_target(i, nb);
        match idx {
            Some(_) => format!("{base_res}-{}", ["unix", "dos", "mac"][idx.unwrap()]),
            None => {
                // Canonical-first: an alias that is itself a group
                // head (no-conversion) wins over appearing inside
                // another group's member list (binary's row).
                if let Some(&(_, group)) = CODING_ALIASES
                    .iter()
                    .find(|&&(n, _)| n == base_res)
                    .or_else(|| {
                        CODING_ALIASES
                            .iter()
                            .find(|&&(_, grp)| grp.iter().any(|m| *m == base_res))
                    })
                {
                    group[0].to_string()
                } else {
                    base_res
                }
            }
        }
    };
    let (hbase, hidx) = eol_split(&head);
    let group: Vec<String> = match hidx {
        // Subsidiary: base's own group with this eol suffix applied,
        // e.g. utf-8-unix → (utf-8-unix mule-utf-8-unix cp65001-unix).
        Some(e) => {
            let hg = CODING_ALIASES
                .iter()
                .find(|&&(n, grp)| n == hbase || grp.iter().any(|m| *m == hbase));
            let suf = ["unix", "dos", "mac"][e];
            match hg {
                // `no-conversion'/`binary' (eol-type nil) have no
                // eol variants at all — `binary-dos' etc. error.
                Some(&(_, base_group)) if hbase != "no-conversion" => {
                    base_group.iter().map(|m| format!("{m}-{suf}")).collect()
                }
                _ => {
                    if i.extra_coding_systems.iter().any(|n| *n == hbase) {
                        vec![head.clone()]
                    } else {
                        return None;
                    }
                }
            }
        }
        None => {
            if let Some(&(_, g)) = CODING_ALIASES
                .iter()
                .find(|&&(n, _)| n == head)
                .or_else(|| {
                    CODING_ALIASES
                        .iter()
                        .find(|&&(_, grp)| grp.iter().any(|m| *m == head))
                })
            {
                g.iter().map(|s| s.to_string()).collect()
            } else if i.extra_coding_systems.iter().any(|n| *n == head) {
                vec![head.clone()]
            } else {
                return None;
            }
        }
    };
    // Fold in runtime aliases that resolve to this exact head —
    // GNU appends them to the resolved system's `aliases' list.
    let mut g = group;
    for (a, t) in &i.extra_coding_aliases {
        if resolve_target(i, t) == head && !g.iter().any(|s| s == a) {
            g.push(a.clone());
        }
    }
    Some(g)
}

pub(super) fn f_coding_system_eol_type(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil means the default (unix ⇒ 0); a non-coding-system or
    // non-symbol argument returns nil rather than signaling.
    match &a[0] {
        Value::Nil => return Ok(Value::Int(0)),
        Value::Sym(_) => {}
        _ => return Ok(Value::Nil),
    };
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    if coding_known(i, &a[0]).is_none() {
        return Ok(Value::Nil);
    }
    if let Some(idx) = coding_fixed_eol(i, &a[0]) {
        return Ok(Value::Int(idx as i128));
    }
    let base = coding_canonical(i, &a[0]);
    // Base system: vector of the three EOL variants.
    Ok(Value::vector(
        ["unix", "dos", "mac"]
            .iter()
            .map(|s| Value::Sym(i.intern(&format!("{base}-{s}"))))
            .collect(),
    ))
}
pub(super) fn f_coding_system_aliases(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU checks CHECK_SYMBOL
    // before consulting the coding-system table.
    // GNU: nil means the no-conversion coding system.
    if a[0].is_nil() {
        return Ok(Value::list(vec![
            Value::Sym(i.q("no-conversion")),
            Value::Sym(i.q("binary")),
        ]));
    }
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    match coding_alias_group(i, &a[0]) {
        Some(group) => Ok(Value::list(
            group.iter().map(|n| Value::Sym(i.intern(n))).collect(),
        )),
        // GNU signals `coding-system-error' on unknown names.
        None => {
            let s = i.q("coding-system-error");
            Err(i.signal_data(s, vec![a[0].clone()]))
        }
    }
}

fn coding_base_extra(i: &Interp, base: &str) -> String {
    // Runtime aliases (`define-coding-system-alias') resolve through
    // their target, recursively, to the static group head or a custom
    // system's own name.
    let mut cur = base.to_string();
    for _ in 0..16 {
        match i.extra_coding_aliases.iter().find(|(x, _)| *x == cur) {
            Some((_, t)) => {
                let (tb, _) = eol_split(t);
                cur = tb.to_string();
            }
            None => break,
        }
    }
    cur
}

pub(super) fn f_coding_system_base(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        // GNU treats nil as `no-conversion'.
        Value::Nil => "no-conversion".to_string(),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    // Validate the original name before stripping a suffix or resolving
    // an alias group, which could otherwise make an unknown name valid.
    if !a[0].is_nil() && coding_known(i, &a[0]).is_none() {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    }
    if i.extra_coding_aliases
        .iter()
        .any(|(alias, _)| *alias == name)
    {
        let base = coding_canonical(i, &a[0]);
        return Ok(Value::Sym(i.intern(&base)));
    }
    // GNU: the bare EOL names `unix'/`dos'/`mac' are subsidiaries of
    // `undecided' (its eol-type vector), not self-based.
    if matches!(name.as_str(), "unix" | "dos" | "mac") {
        return Ok(Value::Sym(i.q("undecided")));
    }
    // Subsidiaries resolve to the base coding system, and alias names
    // resolve to their group's canonical head: GNU maps `cp1256-dos' →
    // `windows-1256', `iso-8859-1' → `iso-latin-1', `mule-utf-8' → `utf-8'.
    let stripped = name
        .strip_suffix("-unix")
        .or_else(|| name.strip_suffix("-dos"))
        .or_else(|| name.strip_suffix("-mac"))
        .unwrap_or(&name);
    let resolved = coding_base_extra(i, stripped);
    let base: &str = &resolved;
    if let Some(&(_, group)) = CODING_ALIASES
        .iter()
        .find(|&&(n, grp)| n == base || grp.iter().any(|m| *m == base))
    {
        return Ok(Value::Sym(i.intern(group[0])));
    }
    Ok(Value::Sym(i.intern(base)))
}

pub(crate) fn f_coding_system_plist(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU stores per-coding-system metadata on the coding-system
    // symbol's plist; we mirror it via the generated
    // `remacs-coding-system-plists' alist keyed by the queried name.
    if i.sym_id(&a[0]).is_none() {
        // GNU treats nil as `no-conversion'.
        if !a[0].is_nil() {
            return Err(i.wrong_type_mut("symbolp", &a[0]));
        }
    }
    let q = if a[0].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[0].clone()
    };
    let Some(_) = coding_known(i, &q) else {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    };
    let table_id = i.q("remacs-coding-system-plists");
    if let Value::Cons(_) = i.symbol_value(table_id) {
        // EOL variants share the base system's plist — GNU's
        // coding-system objects for utf-8-unix/-dos/-mac all carry
        // utf-8's attributes.
        let base = coding_canonical(i, &q);
        let key = Value::Sym(i.intern(&base));
        if let Some(entry) = assq(&i.symbol_value(table_id), &key) {
            if let Value::Cons(pair) = &entry {
                let (_, val) = {
                    let pb = pair.borrow();
                    (pb.car.clone(), pb.cdr.clone())
                };
                return Ok(val);
            }
        }
        return Ok(Value::Nil);
    }
    Ok(Value::Nil)
}

/// `(assq KEY ALIST)' on raw Values.
pub(super) fn assq(alist: &Value, key: &Value) -> Option<Value> {
    let mut cur = alist.clone();
    while let Value::Cons(c) = cur {
        let (car, cdr) = {
            let b = c.borrow();
            (b.car.clone(), b.cdr.clone())
        };
        if let Value::Cons(pair) = &car {
            let (k, _) = {
                let b = pair.borrow();
                (b.car.clone(), b.cdr.clone())
            };
            if let (Value::Sym(a), Value::Sym(b)) = (&k, key) {
                if a == b {
                    return Some(car.clone());
                }
            }
        }
        cur = cdr;
    }
    None
}

pub(super) fn f_coding_system_get(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let prop = match &a[1] {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return Ok(Value::Nil),
    };
    let cs = if a[0].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[0].clone()
    };
    match (coding_known(i, &cs), prop.as_str()) {
        (Some(n), ":name") => Ok(Value::Sym(i.intern(&n))),
        (Some(n), ":coding-type") => Ok(Value::Sym(i.intern(if n.starts_with("utf-8") {
            "utf-8"
        } else {
            "charset"
        }))),
        (Some(_), ":eol-type") => f_coding_system_eol_type(i, a),
        (Some(_), _) => {
            let plist = f_coding_system_plist(i, vec![a[0].clone()].into())?;
            let mut cur = plist;
            while let Value::Cons(c) = cur {
                let (k, rest) = {
                    let b = c.borrow();
                    (b.car.clone(), b.cdr.clone())
                };
                if let (Value::Sym(k), Some(q)) = (&k, i.sym_id(&a[1])) {
                    if *k == q {
                        if let Value::Cons(v) = rest {
                            return Ok(v.borrow().car.clone());
                        }
                        return Ok(Value::Nil);
                    }
                }
                cur = match rest {
                    Value::Cons(c2) => c2.borrow().cdr.clone(),
                    _ => Value::Nil,
                };
            }
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}

/// GNU's `add-to-coding-system-list' (mule.el): sorted insert of NAME
/// into the `coding-system-list' variable using `coding-system-lessp'
/// (mnemonic-char ordering with no-conversion/emacs-mule/undecided
/// special-cased), inserting AFTER equal keys — subsidiaries land in
/// parent, mac, dos, unix order when registered that way.  Falls back
/// to a plain append while the Lisp defun isn't bound yet (pre-mule
/// init) or for names without a mnemonic.  No-op when already a member.
pub(crate) fn add_to_coding_system_list(i: &mut Interp, name: &str) -> EvalResult {
    let sid = i.intern(name);
    let csl = i.q("coding-system-list");
    let cur = i.symbol_value(csl).list_to_vec().unwrap_or_default();
    if cur.iter().any(|v| matches!(v, Value::Sym(s) if *s == sid)) {
        return Ok(Value::Nil);
    }
    // Prefer the real Lisp function (mule.el loads it early): it knows
    // mnemonics through `coding-system-plist'.
    let add = i.q("add-to-coding-system-list");
    let fbound = !matches!(
        i.symbol_function(add),
        Value::Sym(s) if s == crate::lisp::sym::UNBOUND
    );
    if fbound {
        i.apply(&Value::Sym(add), vec![Value::Sym(sid)])?;
        return Ok(Value::Nil);
    }
    let mut items = cur;
    items.push(Value::Sym(sid));
    i.set_symbol(csl, Value::list(items))?;
    Ok(Value::Nil)
}

pub(super) fn f_coding_system_put(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: SYMBOLP checks CS then PROP; unknown CS → coding-system-error;
    // Existing properties update in place; new properties append.
    let cs = match &a[0] {
        Value::Sym(_) => a[0].clone(),
        other => return Err(i.wrong_type_mut("symbolp", other)),
    };
    if i.sym_id(&a[1]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[1]));
    }
    if coding_known(i, &cs).is_none() {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![cs]));
    }
    let table_id = i.q("remacs-coding-system-plists");
    let key = {
        let sid = i.sym_id(&a[0]).unwrap();
        let resolved = coding_base_extra(i, &i.symbol_name(sid).to_string());
        let (base, _) = eol_split(&resolved);
        Value::Sym(i.intern(base))
    };
    let mut cur = i.symbol_value(table_id);
    loop {
        let (car, cdr) = match &cur {
            Value::Cons(c) => {
                let b = c.borrow();
                (b.car.clone(), b.cdr.clone())
            }
            _ => break,
        };
        if let Value::Cons(pair) = &car {
            let (pk, pl) = {
                let pb = pair.borrow();
                (pb.car.clone(), pb.cdr.clone())
            };
            if eq_values(&pk, &key) {
                pair.borrow_mut().cdr = crate::lisp::eval::plist_put_v(&pl, &a[1], a[2].clone());
                return Ok(a[2].clone());
            }
        }
        cur = cdr;
    }
    // Known coding system with no plist entry yet — create one.
    let entry = Value::cons(
        key,
        Value::cons(a[1].clone(), Value::cons(a[2].clone(), Value::Nil)),
    );
    let new_alist = Value::cons(entry, i.symbol_value(table_id));
    i.set_symbol(table_id, new_alist)?;
    Ok(a[2].clone())
}

pub(super) fn f_terminal_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // Optional TERMINAL is terminal-live-p-checked.
    if let Some(v) = a.first() {
        match v {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    Ok(i.terminal_coding.clone())
}

pub(super) fn f_keyboard_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if let Some(v) = a.first() {
        match v {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    Ok(i.keyboard_coding.clone())
}

pub(super) fn f_unencodable_char_position(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Funencodable_char_position (coding.c):
    // check_coding_system runs FIRST (non-symbol → `symbolp', unknown →
    // `coding-system-error'); then STRING non-nil switches to string
    // mode (substring-style bounds), else buffer mode
    // (`integer-or-marker-p'). Nil coding selects undecided.
    if !a[2].is_nil() {
        check_coding(i, &a[2])?;
    }
    let coding = if a[2].is_nil() {
        Value::Sym(i.q("undecided"))
    } else {
        a[2].clone()
    };
    // Raw-text coding accepts every character, before validating bounds.
    if eol_split(&coding_canonical(i, &coding)).0 == "raw-text" {
        return Ok(Value::Nil);
    }
    // A char encodes iff some charset of the coding system encodes it;
    // an empty charset list (no-conversion) accepts everything.
    let cslist = crate::lisp::builtins::charset::coding_system_charset_names(i, &coding);
    let csnames: Vec<String> = cslist
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| match v {
            Value::Sym(s) => Some(i.symbol_name(*s)),
            _ => None,
        })
        .collect();
    let unencodable = |i: &Interp, ch: crate::lisp::text::EmacsChar| {
        !csnames.is_empty()
            && !csnames.iter().any(|n| {
                crate::lisp::builtins::charset::charset_encode(i, n, ch.code() as i64).is_some()
            })
    };
    let mut hits: Vec<i128> = Vec::new();
    match a.get(4) {
        Some(Value::Str(s)) => {
            let sv: Vec<crate::lisp::value::EmacsChar> = s.borrow().chars().collect();
            let (pos, end) =
                crate::lisp::builtins::strfn::validate_subarray(i, &a[4], &a[0], &a[1], sv.len())?;
            if i.is_unibyte_str(s) {
                return Ok(Value::Nil);
            }
            for p in pos..end {
                if unencodable(i, sv[p as usize]) {
                    hits.push(p as i128);
                }
            }
        }
        Some(Value::Nil) | None => {
            let start = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
            let end = crate::lisp::builtins::want_int_or_mark(i, &a[1])?;
            let Some(b) = i.current_buffer_ref() else {
                return Ok(Value::Nil);
            };
            let bb = b.borrow();
            let (lo, hi) = (bb.begv as i128 + 1, bb.zv.min(bb.text_len()) as i128 + 1);
            if start < lo || start > end || end > hi {
                let bv = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
                drop(bb);
                return Err(
                    i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![bv, a[0].clone(), a[1].clone()])
                );
            }
            for p in start..end {
                if unencodable(i, bb.text.char_at((p - 1) as usize)) {
                    hits.push(p);
                }
            }
        }
        Some(other) => return Err(i.wrong_type_mut("stringp", other)),
    }
    // COUNT is checked lazily: nil → the first position (or nil);
    // non-nil forces a list, natnum → at most N entries (0 → all),
    // anything else signals `wholenump' — but only when a hit exists.
    if hits.is_empty() {
        return Ok(Value::Nil);
    }
    match &arg(&a, 3) {
        Value::Nil => Ok(Value::Int(hits[0])),
        Value::Int(n) if *n >= 0 => {
            let take = if *n == 0 { usize::MAX } else { *n as usize };
            Ok(Value::list(
                hits.iter().take(take).map(|p| Value::Int(*p)).collect(),
            ))
        }
        other => Err(i.wrong_type_mut("wholenump", other)),
    }
}

pub(super) fn f_compose_region_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (start, end) = (
        crate::lisp::builtins::want_int_or_mark(i, &a[0])?,
        crate::lisp::builtins::want_int_or_mark(i, &a[1])?,
    );
    let (begv, zv, bufv) = i
        .current_buffer_ref()
        .map(|b| {
            let (bb, bv) = (b.borrow(), Value::Buffer(b.clone()));
            (bb.begv as i128 + 1, bb.zv as i128 + 1, bv)
        })
        .unwrap_or((1, 1, Value::Nil));
    if start < begv || end > zv || start > end {
        // GNU's args-out-of-range data is (BUFFER START END).
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![bufv, a[0].clone(), a[1].clone()],
        ));
    }
    // GNU Fcompose_region_internal type-checks COMPONENTS
    // (fixnum/cons/string pass, other non-nil → `vectorp'), then
    // compose_text installs `composition' = ((WIDTH . COMPONENTS)
    // . MODIFICATION-FUNC) over the range.
    if let Some(v) = a.get(2) {
        if v.truthy()
            && !matches!(
                v,
                Value::Int(_) | Value::Cons(_) | Value::Str(_) | Value::Vec(_)
            )
        {
            return Err(i.wrong_type_mut("vectorp", v));
        }
    }
    let width = Value::Int(end - start);
    let head = match a.get(2) {
        Some(v) if v.truthy() => Value::cons(width, v.clone()),
        _ => width,
    };
    let prop_val = match a.get(3) {
        Some(v) if v.truthy() => Value::cons(head, v.clone()),
        _ => Value::list(vec![head]),
    };
    let comp_sym = Value::Sym(i.q("composition"));
    crate::buffer::primitives::f_put_text_property(
        i,
        vec![a[0].clone(), a[1].clone(), comp_sym, prop_val, Value::Nil].into(),
    )?;
    Ok(Value::Nil)
}

pub(super) fn f_compose_string_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let (start, end) = (want_int(i, &a[1])?, want_int(i, &a[2])?);
    if start < 0 || end > s.chars().count() as i128 || start > end {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![a[0].clone(), a[1].clone(), a[2].clone()],
        ));
    }
    // GNU Fcompose_string_internal installs the `composition' property
    // over [START,END): value ((WIDTH . COMPONENTS) . MODIFICATION-STRING)
    // where WIDTH = END-START and COMPONENTS is the raw COMPONENTS arg
    // (absent → just (WIDTH)); MODIFICATION-STRING extends the head.
    let width = Value::Int(end - start);
    // WIDTH . COMPONENTS is a single cons even when COMPONENTS is nil.
    let head = Value::cons(width, a.get(3).cloned().unwrap_or(Value::Nil));
    let prop_val = match a.get(4) {
        // GNU: modification-string cdrs the (WIDTH . COMPONENTS) entry —
        // no list wrap; absent → the entry is a one-element list.
        Some(v) if v.truthy() => Value::cons(head, v.clone()),
        _ => Value::list(vec![head]),
    };
    let comp_sym = Value::Sym(i.q("composition"));
    crate::buffer::primitives::f_put_text_property(
        i,
        vec![a[1].clone(), a[2].clone(), comp_sym, prop_val, a[0].clone()].into(),
    )?;
    Ok(a[0].clone())
}

pub(super) fn f_register_ccl_program(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_sym(i, &a[0])?;
    let elems = match &a[1] {
        Value::Vec(v) => v.borrow().clone(),
        other => return Err(i.wrong_type_mut("vectorp", other)),
    };
    if elems.len() < 3 || !elems.iter().all(|e| matches!(e, Value::Int(_))) {
        // GNU ccl.c Fregister_ccl_program: "Error in CCL program"
        // (the execute path's "Invalid CCL program" is different).
        return Err(i.signal_data(sym::ERROR, vec![Value::string("Error in CCL program")]));
    }
    let prop = i.q("ccl-program");
    if let Value::Int(idx) = i.get_prop(name, prop) {
        return Ok(Value::Int(idx));
    }
    let idx = i.ccl_program_count;
    i.ccl_program_count += 1;
    i.put_prop(name, prop, Value::Int(idx as i128));
    Ok(Value::Int(idx as i128))
}

pub(super) fn f_ccl_program_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU: nil for non-symbols
    // too — only a symbol with a `ccl-program' property counts.
    let Some(name) = i.sym_id(&a[0]) else {
        return Ok(Value::Nil);
    };
    let prop = i.q("ccl-program");
    Ok(if i.get_prop(name, prop).is_nil() {
        Value::Nil
    } else {
        Value::t()
    })
}

pub(super) fn ccl_prog_of(i: &mut Interp, v: &Value) -> Result<(), Flow> {
    // GNU: PROGRAM is a symbol whose `ccl-program' prop is a vector,
    // or a vector itself; anything else signals the plain error.
    let prog = match v {
        Value::Sym(s) => {
            let prop = i.q("ccl-program");
            i.get_prop(*s, prop)
        }
        other => other.clone(),
    };
    if matches!(prog, Value::Vec(_)) {
        Ok(())
    } else {
        Err(i.signal_data(sym::ERROR, vec![Value::string("Invalid CCL program")]))
    }
}

pub(super) fn f_ccl_execute(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    ccl_prog_of(i, &a[0])?;
    Ok(Value::Nil)
}

pub(super) fn f_ccl_execute_on_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU validates the CCL program first (Fccl_execute_on_string ->
    // ccl_driver before CHECK_STRING lands via the status register);
    // invalid program reports the plain "Invalid CCL program" error.
    ccl_prog_of(i, &a[0])?;
    let s = want_string(i, &a[1])?;
    // STATUS vector: 10 slots of int 0.
    if let Value::Vec(v) = &a[2] {
        for x in v.borrow_mut().iter_mut().take(10) {
            *x = Value::Int(0);
        }
    }
    Ok(Value::string(s))
}

pub(super) fn f_register_code_conversion_map(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let name = want_sym(i, &a[0])?;
    if !matches!(a[1], Value::Vec(_)) {
        return Err(i.wrong_type_mut("vectorp", &a[1]));
    }
    let prop = i.q("code-conversion-map");
    if let Value::Int(idx) = i.get_prop(name, prop) {
        return Ok(Value::Int(idx));
    }
    let idx = i.code_conv_map_count;
    i.code_conv_map_count += 1;
    i.put_prop(name, prop, Value::Int(idx as i128));
    Ok(Value::Int(idx as i128))
}

pub(super) fn f_zlib_available_p(_i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::t())
}

pub(super) fn f_zlib_decompress_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    use std::io::Read;
    let (start, end) = (
        crate::lisp::builtins::want_int_or_mark(i, &a[0])?,
        crate::lisp::builtins::want_int_or_mark(i, &a[1])?,
    );
    let buf = match i.current_buffer_ref() {
        Some(b) => b,
        None => return Err(i.signal_data(sym::ERROR, vec![Value::string("No buffer")])),
    };
    let bytes = {
        let bb = buf.borrow();
        let (begv, zv) = (bb.begv as i128 + 1, bb.zv as i128 + 1);
        if start < begv || end > zv || start > end {
            return Err(i.signal_data(sym::ARGS_OUT_OF_RANGE, vec![a[0].clone(), a[1].clone()]));
        }
        let t = bb.text.text();
        let chars: Vec<crate::lisp::value::EmacsChar> = t.chars().collect();
        let lo = (start - 1).max(0) as usize;
        let hi = (end - 1).min(chars.len() as i128) as usize;
        // Unibyte buffers represent bytes above ASCII as byte8 proxy chars.
        let mut bytes = Vec::new();

        for &c in &chars[lo..hi] {
            if let Some(byte) = c.byte8() {
                bytes.push(byte);
            } else if c.code() < 256 {
                bytes.push((c).code() as u8);
            } else {
                c.encode_internal(&mut bytes);
            }
        }
        bytes
    };
    // Try gzip first, then raw zlib.
    let mut decoded: Option<Vec<u8>> = None;
    {
        let mut d = flate2::read::GzDecoder::new(&bytes[..]);
        let mut out = Vec::new();
        if d.read_to_end(&mut out).is_ok() {
            decoded = Some(out);
        }
    }
    if decoded.is_none() {
        let mut d = flate2::read::ZlibDecoder::new(&bytes[..]);
        let mut out = Vec::new();
        if d.read_to_end(&mut out).is_ok() {
            decoded = Some(out);
        }
    }
    match decoded {
        Some(out) => {
            let text = String::from_utf8_lossy(&out).into_owned();
            let lo = (start - 1).max(0) as usize;
            let hi = (end - 1).max(lo as i128) as usize;
            crate::buffer::primitives::chg_delete(i, lo, hi)?;
            crate::buffer::primitives::chg_insert(i, lo, &text)?;
            Ok(Value::t())
        }
        None => Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("Malformed or misplaced compressed data")],
        )),
    }
}

pub(super) fn f_insert_byte(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let byte = crate::lisp::builtins::want_fixnum(i, &a[0])?;
    // GNU insert_byte: CHECK_FIXNUM_RANGE(byte, 0, 255) — range checked
    // BEFORE COUNT is coerced; datum carries BYTE MIN MAX.
    if !(0..=255).contains(&byte) {
        return Err(i.signal_data(
            sym::ARGS_OUT_OF_RANGE,
            vec![a[0].clone(), Value::Int(0), Value::Int(255)],
        ));
    }
    let count = crate::lisp::builtins::want_fixnum(i, &a[1])?;
    // GNU insert_byte (insdel.c): in a multibyte buffer bytes ≥0x80 are
    // inserted as the raw-byte escape char BYTE8_TO_CHAR(byte) =
    // byte - 0x80 + 0x3FFF80 = 0x3FFF00 + byte (eight-bit-graphic
    // charset; `lisp_char' returns the proxy char); bytes <0x80 are
    // literal.  Unibyte buffers store the byte as-is (char byte).
    let ems = i.q("enable-multibyte-characters");
    let mb = i.symbol_value(ems).truthy();
    let code = if mb && byte >= 0x80 {
        byte as u32 + 0x3FFF00
    } else {
        byte as u32
    };
    let ch = crate::lisp::value::lisp_char(code).unwrap_or('\u{fffd}'.into());
    let s: crate::lisp::text::EmacsText = std::iter::repeat_n(ch, count.max(0) as usize).collect();
    if a.get(2).is_some_and(Value::truthy) {
        crate::buffer::primitives::chg_insert_and_inherit_pt(i, &s)?;
    } else {
        crate::buffer::primitives::chg_insert_pt(i, &s, false)?;
    }
    Ok(Value::Nil)
}

pub(super) fn f_set_binary_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let sid = want_sym(i, &a[0])?;
    let name = i.symbol_name(sid);
    if !matches!(name.as_str(), "stdin" | "stdout" | "stderr") {
        // GNU fileio.c: xsignal2 'error ("unsupported stream" STREAM) —
        // the datum carries the stream object, not its name.
        return Err(i.signal_data(
            sym::ERROR,
            vec![Value::string("unsupported stream"), a[0].clone()],
        ));
    }
    // POSIX: always binary; value is the previous mode (non-nil).
    Ok(Value::t())
}

/// `set-input-meta-mode' — META arg is ignored on our model; GNU
/// checks the optional TERMINAL with terminal-live-p.
pub(super) fn f_set_input_meta_mode(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 1) {
        Value::Nil => Ok(Value::Nil),
        w if crate::editor::is_terminal(i, &w) => Ok(Value::Nil),
        other => Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
}

/// Shared body for `set-terminal-coding-system' /
/// `set-keyboard-coding-system'.  GNU `set-terminal-coding-system'
/// decodes the optional TERMINAL arg FIRST (terminal-live-p on a bad
/// arg fires before the CODING-SYSTEM checks); the keyboard variant
/// checks its CODING-SYSTEM arg first.
pub(super) fn set_coding_system(
    i: &mut Interp,
    a: crate::lisp::Argv,
    ret_name: bool,
) -> EvalResult {
    if !ret_name {
        match arg(&a, 1) {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, &w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", &other)),
        }
    }
    match &a[0] {
        // GNU: nil resets — terminal to nil, keyboard to no-conversion;
        // both return nil.
        Value::Nil => {
            if ret_name {
                i.keyboard_coding = Value::Sym(i.q("no-conversion"));
            } else {
                i.terminal_coding = Value::Nil;
            }
            return Ok(Value::Nil);
        }
        Value::Sym(_) => {
            if i.sym_id(&a[0]).is_none() {
                return Err(i.wrong_type_mut("symbolp", &a[0]));
            }
            if coding_known(i, &a[0]).is_none() {
                let s = i.q("coding-system-error");
                return Err(i.signal_data(s, vec![a[0].clone()]));
            }
        }
        other => return Err(i.wrong_type_mut("symbolp", other)),
    }
    match arg(&a, 1) {
        Value::Nil => {}
        w if crate::editor::is_terminal(i, &w) => {}
        other => return Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
    if !ret_name {
        // `set-terminal-coding-system' stores the given name and
        // returns nil.
        i.terminal_coding = a[0].clone();
        return Ok(Value::Nil);
    }
    // GNU's keyboard coding canonicalizes aliases and gains the
    // platform EOL suffix.
    let name = match &a[0] {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => unreachable!(),
    };
    let canon = coding_alias_group(i, &a[0])
        .and_then(|g| g.first().cloned())
        .unwrap_or(name);
    let bare = canon
        .strip_suffix("-unix")
        .or_else(|| canon.strip_suffix("-dos"))
        .or_else(|| canon.strip_suffix("-mac"))
        .is_some();
    let out = if bare { canon } else { format!("{canon}-unix") };
    let sym = Value::Sym(i.intern(&out));
    i.keyboard_coding = sym.clone();
    Ok(sym)
}

pub(super) fn f_set_terminal_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    set_coding_system(i, a, false)
}

pub(super) fn f_set_keyboard_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU's mule.el Lisp defun: check the coding type for suitability,
    // save/restore the meta-mode flag around it, change the EOL
    // conversion to unix, then run the internal setter and mirror the
    // result into the `keyboard-coding-system' variable.
    if let Some(v) = a.get(1) {
        match v {
            Value::Nil => {}
            w if crate::editor::is_terminal(i, w) => {}
            other => return Err(i.wrong_type_mut("terminal-live-p", other)),
        }
    }
    if !matches!(a[0], Value::Sym(_) | Value::Nil) {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    // nil means "disable decoding" → raw-text type → accept-8-bit.
    let cs_val = if a[0].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[0].clone()
    };
    if coding_known(i, &cs_val).is_none() {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![cs_val.clone()]));
    }
    let coding_type =
        crate::lisp::builtins::charset::f_coding_system_type(i, vec![cs_val.clone()].into())?;
    let type_name = match &coding_type {
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => String::new(),
    };
    let accept_8_bit = if ["raw-text", "charset", "utf-8", "shift-jis", "big5", "ccl"]
        .contains(&type_name.as_str())
    {
        true
    } else if type_name == "iso-2022" {
        // `(or (memq '7-bit flags) (setq accept-8-bit t))'
        let flags_prop = i.q(":flags");
        let flags = f_coding_system_get(i, vec![cs_val.clone(), Value::Sym(flags_prop)].into())?;
        !flags
            .list_to_vec()
            .unwrap_or_default()
            .iter()
            .any(|v| matches!(v, Value::Sym(s) if i.symbol_name(*s) == "7-bit"))
    } else {
        let name = match &a[0] {
            Value::Sym(s) => i.symbol_name(*s).to_string(),
            _ => "no-conversion".to_string(),
        };
        return Err(i.error(format!("Unsupported coding system for keyboard: {name}")));
    };
    let pkey = i.q("keyboard-coding-saved-meta-mode");
    let saved_meta = i
        .terminal_params
        .iter()
        .find(|(k, _)| matches!(k, Value::Sym(s) if *s == pkey))
        .map(|(_, v)| v.clone())
        .unwrap_or(Value::Nil);
    if accept_8_bit {
        if saved_meta.is_nil() {
            // Save `(nth 2 (current-input-mode))' — the meta flag —
            // under the saved-meta parameter; set meta-mode 8.
            let meta_flag = Value::t();
            i.terminal_params
                .retain(|(k, _)| !matches!(k, Value::Sym(s) if *s == pkey));
            i.terminal_params
                .push((Value::Sym(pkey), Value::list(vec![meta_flag])));
        }
        let _ = f_set_input_meta_mode(i, vec![Value::Int(8)].into());
    } else if saved_meta.truthy() {
        let meta = saved_meta
            .list_to_vec()
            .ok()
            .and_then(|v| v.first().cloned())
            .unwrap_or(Value::Nil);
        let _ = f_set_input_meta_mode(i, vec![meta].into());
        i.terminal_params
            .retain(|(k, _)| !matches!(k, Value::Sym(s) if *s == pkey));
    }
    // EOL: `(coding-system-change-eol-conversion CS 'unix)' — on
    // variant-less systems this returns CS unchanged (no bogus
    // `-unix' names).
    let f = i.q("coding-system-change-eol-conversion");
    let unix_id = i.q("unix");
    let set = i.apply(&Value::Sym(f), vec![cs_val, Value::Sym(unix_id)])?;
    i.keyboard_coding = set.clone();
    let var = i.q("keyboard-coding-system");
    let _ = i.set_symbol_default(var, set.clone());
    Ok(set)
}

/// Detect the byte sequence represented by a Lisp string, rather than the
/// UTF-8 backing storage used by Rust.
pub(super) fn f_detect_coding_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &a[0])?;
    let unibyte = matches!(&a[0], Value::Str(r) if i.is_unibyte_str(r));
    let input: Vec<i32> = s
        .chars()
        .map(|c| {
            if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
                b as i32
            } else if unibyte || c.is_ascii() {
                c.code() as i32
            } else {
                -(crate::lisp::value::lisp_char_code(c) as i32)
            }
        })
        .collect();
    crate::lisp::builtins::coding_detection::detect(i, &input, !unibyte, arg(&a, 1).truthy())
}

pub(super) fn coding_region_source(
    i: &mut Interp,
    a: &[Value],
) -> Result<(Vec<i128>, Value), Flow> {
    let mut positions = Vec::new();
    for v in &a[..2] {
        let position = match v {
            Value::Int(n) if v.fixnump() => *n,
            Value::Int(_) | Value::BigInt(_) => {
                let buffer = i.buffer_value(i.current_buffer).unwrap_or(Value::Nil);
                return Err(i.signal_data(
                    sym::ARGS_OUT_OF_RANGE,
                    vec![buffer, a[0].clone(), a[1].clone()],
                ));
            }
            Value::Marker(m) if m.borrow().buffer.is_some() => m.borrow().position as i128 + 1,
            Value::Marker(_) => return Err(i.error("Marker does not point anywhere")),
            v => return Err(i.wrong_type_mut("integer-or-marker-p", v)),
        };
        positions.push(position);
    }
    positions.sort();
    let source = crate::buffer::primitives::f_buffer_substring(
        i,
        positions
            .iter()
            .map(|p| Value::Int(*p))
            .collect::<Vec<_>>()
            .into(),
    )?;
    Ok((positions, source))
}

pub(super) fn f_detect_coding_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let (_, source) = coding_region_source(i, &a)?;
    f_detect_coding_string(i, vec![source, arg(&a, 2)].into())
}

/// Canonical coding-system name for the arg (alias-group head, EOL
/// suffix stripped).
pub(crate) fn coding_canonical(i: &Interp, v: &Value) -> String {
    let mut name = match v {
        Value::Nil => "no-conversion".to_string(),
        Value::Sym(s) => i.symbol_name(*s).to_string(),
        _ => return String::new(),
    };
    for _ in 0..32 {
        let base = eol_split(&name).0.to_string();
        if let Some((_, target)) = i
            .extra_coding_aliases
            .iter()
            .find(|(n, _)| *n == name || *n == base)
        {
            name = target.clone();
            continue;
        }
        return CODING_ALIASES
            .iter()
            .find(|&&(n, _)| n == base)
            .or_else(|| {
                CODING_ALIASES
                    .iter()
                    .find(|&&(_, group)| group.contains(&base.as_str()))
            })
            .map(|&(_, group)| eol_split(group[0]).0.to_string())
            .unwrap_or(base);
    }
    eol_split(&name).0.to_string()
}

/// Follow aliases before selecting an EOL subsidiary; a bare base uses
/// automatic EOL detection when decoding and LF when encoding.
pub(crate) fn coding_fixed_eol(i: &Interp, v: &Value) -> Option<usize> {
    let Value::Sym(s) = v else { return Some(0) };
    let mut name = i.symbol_name(*s).to_string();
    for _ in 0..32 {
        if let Some((_, target)) = i.extra_coding_aliases.iter().find(|(n, _)| *n == name) {
            name = target.clone();
        } else {
            if let Some(eol) = eol_split(&name).1 {
                return Some(eol);
            }
            return match name.as_str() {
                "unix" | "no-conversion" | "binary" => Some(0),
                "dos" => Some(1),
                "mac" => Some(2),
                _ => None,
            };
        }
    }
    None
}

pub(super) fn encode_coding_text(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    encode_process_text(i, a, None, false)
}

/// Pure conversion shared by strings and process encoders. Process callers
/// keep ISO designation state and suppress a signature already emitted.
pub(crate) fn encode_process_text(
    i: &mut Interp,
    a: crate::lisp::Argv,
    iso: Option<&mut crate::lisp::builtins::coding_iso2022::EncoderState>,
    signature_written: bool,
) -> EvalResult {
    let text = want_string(i, &a[0])?;
    f_check_coding_system(i, vec![a[1].clone()].into())?;
    let canonical = coding_canonical(i, &a[1]);
    let coding = Value::Sym(i.intern(&canonical));
    let inhibit = i.q("inhibit-eol-conversion");
    let eol = if i.symbol_value(inhibit).truthy() {
        0
    } else {
        coding_fixed_eol(i, &a[1]).unwrap_or(0)
    };
    let unibyte = matches!(&a[0], Value::Str(r) if i.is_unibyte_str(r));
    let intervals = match &a[0] {
        Value::Str(r) => i.str_props(r).to_vec(),
        _ => Vec::new(),
    };
    let charset = i.q("charset");
    let mut preferred = Vec::new();
    let mut input = crate::lisp::text::EmacsText::default();
    for (pos, ch) in text.chars().enumerate() {
        let preference = intervals
            .iter()
            .filter(|(start, end, _)| *start <= pos && pos < *end)
            .find_map(|(_, _, pl)| {
                pl.chunks_exact(2).find_map(|pair| {
                    if matches!(pair[0], Value::Sym(id) if id == charset) {
                        i.sym_id(&pair[1]).map(|id| i.symbol_name(id).to_string())
                    } else {
                        None
                    }
                })
            });
        preferred.push(preference.clone());
        if ch == '\n' && eol != 0 {
            input.push('\r');
            if eol == 1 {
                input.push('\n');
                preferred.push(preference);
            }
        } else if unibyte && (128..=255).contains(&ch.code()) {
            input.push(crate::lisp::value::lisp_char(0x3fff00 + (ch).code() as u32).unwrap());
        } else {
            input.push(ch);
        }
    }
    let plist = f_coding_system_plist(i, vec![coding.clone()].into())?;
    let default_key = i.q(":default-char");
    let default = crate::lisp::eval::plist_get(&plist, default_key)
        .int()
        .unwrap_or(32) as u32;
    let iso = if let Some(state) = iso {
        crate::lisp::builtins::coding_iso2022::encode_stream(
            i, &coding, &input, false, &preferred, state,
        )
    } else {
        crate::lisp::builtins::coding_iso2022::encode_with_preferences(
            i, &coding, &input, true, &preferred,
        )
    };
    let bytes = if let Some(mut bytes) =
        crate::lisp::builtins::coding_unicode::encode(&canonical, &input, default)
    {
        if signature_written {
            if (canonical == "utf-16" || canonical.ends_with("-with-signature"))
                && canonical.starts_with("utf-16")
            {
                bytes.drain(..2);
            } else if matches!(canonical.as_str(), "utf-8-auto" | "utf-8-with-signature") {
                bytes.drain(..3);
            }
        }
        bytes
    } else if canonical == "chinese-hz" {
        encode_hz(i, &input)
    } else if let Some(bytes) = iso {
        bytes
    } else if let Some(bytes) =
        crate::lisp::builtins::coding_legacy::encode(i, &coding, &input, true)
    {
        bytes
    } else {
        let plist = f_coding_system_plist(i, vec![coding].into())?;
        let dc = crate::lisp::eval::plist_get(&plist, i.q(":default-char"));
        let default = dc.int().unwrap_or(32) as u8;
        input
            .chars()
            .flat_map(|ch| {
                if let Some(b) = crate::lisp::value::eight_bit_byte(ch) {
                    vec![b]
                } else {
                    encode_one_char(&canonical, ch.code()).unwrap_or_else(|| vec![default])
                }
            })
            .collect()
    };
    let value = Value::string(bytes.iter().map(|b| *b as char).collect::<String>());
    if let Value::Str(r) = &value {
        i.mark_unibyte(r);
    }
    Ok(value)
}

pub(super) fn encode_hz(i: &Interp, text: &crate::lisp::text::EmacsStr) -> Vec<u8> {
    let mut out = Vec::new();
    let mut shifted = false;
    for ch in text.chars() {
        let c = crate::lisp::value::lisp_char_code(ch);
        let code = if c >= 128 {
            crate::lisp::builtins::charset::charset_encode(i, "chinese-gb2312", c as i64)
        } else {
            None
        };
        if let Some(code) = code {
            if !shifted {
                out.extend_from_slice(b"~{");
                shifted = true;
            }
            out.extend_from_slice(&[(code >> 8) as u8, code as u8]);
        } else {
            if shifted {
                out.extend_from_slice(b"~}");
                shifted = false;
            }
            if c == 126 {
                out.extend_from_slice(b"~~");
            } else if c < 128 {
                out.push(c as u8);
            } else {
                out.extend_from_slice(
                    format!(
                        "{}{:0width$X}",
                        if c < 65536 { "\\u" } else { "\\U" },
                        c,
                        width = if c < 65536 { 4 } else { 8 }
                    )
                    .as_bytes(),
                );
            }
        }
    }
    if shifted {
        out.extend_from_slice(b"~}");
    }
    out
}

pub(super) fn decode_hz(
    i: &mut Interp,
    bytes: &[u8],
) -> Result<(crate::lisp::text::EmacsText, Vec<(usize, usize, String)>), Flow> {
    // HZ's Lisp post-read conversion operates after UTF-8 decoding, and
    // only converts complete contiguous GB pairs following a ~{ marker.
    let chars: Vec<crate::lisp::text::EmacsChar> =
        crate::lisp::builtins::coding_legacy::internal_decode(bytes)
            .chars()
            .collect();
    let mut out = crate::lisp::text::EmacsText::default();
    let mut spans = Vec::new();
    let mut pos = 0;
    let mut at = 0;
    let continuation = i
        .intern_soft("decode-hz-line-continuation")
        .is_some_and(|id| i.symbol_value(id).truthy());
    while at < chars.len() {
        if chars[at] == '~' && at + 1 == chars.len() {
            return Err(i.wrong_type_mut("wholenump", &Value::Nil));
        }
        if chars[at] == '~' && at + 1 < chars.len() {
            match chars[at + 1].code() {
                123 => {
                    at += 2;
                    let start = pos;
                    while at + 1 < chars.len()
                        && (33..=125).contains(&chars[at].code())
                        && (33..=126).contains(&chars[at + 1].code())
                    {
                        let code = ((chars[at].code() as i64) << 8) | chars[at + 1].code() as i64;
                        if let Some(c) = crate::lisp::builtins::charset::charset_decode(
                            i,
                            "chinese-gb2312",
                            code,
                        ) {
                            out.push(
                                crate::lisp::value::lisp_char(c as u32)
                                    .unwrap_or_else(|| crate::lisp::value::mule_seq_char(c as u32)),
                            );
                            pos += 1;
                            at += 2;
                        } else {
                            break;
                        }
                    }
                    if pos > start {
                        spans.push((start, pos, "chinese-gb2312".to_string()));
                    }
                    if chars.get(at).is_some_and(|c| *c == '~')
                        && chars.get(at + 1).is_some_and(|c| *c == '}')
                    {
                        at += 2;
                    }
                    continue;
                }
                126 => {
                    out.push('~');
                    pos += 1;
                    at += 2;
                    continue;
                }
                10 if continuation => {
                    at += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(chars[at]);
        pos += 1;
        at += 1;
    }
    Ok((out, spans))
}

pub(super) fn decode_coding_bytes(
    i: &mut Interp,
    coding: &Value,
    canonical: &str,
    bytes: &[u8],
    force_conversion: bool,
) -> Result<Value, Flow> {
    crate::lisp::builtins::coding_legacy::validate_decode(i, coding, bytes)?;
    let converted = if canonical.starts_with("utf-16") {
        let units: Vec<i32> = bytes.iter().map(|b| *b as i32).collect();
        let name = crate::lisp::builtins::coding_unicode::resolve(canonical, &units, false);
        let text = crate::lisp::builtins::coding_unicode::decode_utf16(&name, &units);
        Some((
            crate::lisp::builtins::coding_unicode::normalize_eol(
                &text,
                coding_fixed_eol(i, coding),
            )
            .0,
            Vec::new(),
        ))
    } else if matches!(canonical, "utf-8-with-signature" | "utf-8-auto") {
        let bytes = bytes.strip_prefix(&[239, 187, 191]).unwrap_or(bytes);
        Some((decode_file_utf8(bytes), Vec::new()))
    } else if canonical == "chinese-hz" {
        Some(decode_hz(i, bytes)?)
    } else if let Some(decoded) = crate::lisp::builtins::coding_iso2022::decode(i, coding, bytes) {
        Some(decoded)
    } else {
        crate::lisp::builtins::coding_legacy::decode(i, coding, bytes, force_conversion)
    };
    let (text, spans, metadata) = match converted {
        Some((text, spans)) => (text, spans, true),
        None => (
            if matches!(canonical, "utf-8" | "utf-8-auto" | "undecided") {
                decode_file_utf8(bytes)
            } else {
                decode_bytes(canonical, bytes).into()
            },
            Vec::new(),
            false,
        ),
    };
    let value = Value::string_with_kind(text, true);
    if let Value::Str(r) = &value {
        i.mark_multibyte(r);
        if metadata {
            let charset = i.q("charset");
            let props = spans
                .into_iter()
                .map(|(start, end, name)| {
                    (
                        start,
                        end,
                        vec![Value::Sym(charset), Value::Sym(i.intern(&name))],
                    )
                })
                .collect();
            i.set_str_props(r, props);
        } else {
            attach_decode_charset_props(i, r, canonical);
        }
    }
    Ok(value)
}

/// Decode bytes to chars for CANONICAL coding names — the inverse of
/// `encode_one_char' using the same generated tables.
pub(super) fn decode_bytes(canonical: &str, bytes: &[u8]) -> String {
    use crate::lisp::builtins::enc_tables::*;
    let utf8ish = matches!(
        canonical,
        "utf-8"
            | "mule-utf-8"
            | "cp65001"
            | "utf-8-auto"
            | "utf-8-emacs"
            | "utf-8-hfs"
            | "utf-8-nfd"
            | "utf-8-with-signature"
            | "prefer-utf-8"
            | "no-conversion"
            | "raw-text"
            | "binary"
            | "no-conversion-multibyte"
            | "undecided"
    );
    if utf8ish {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if matches!(canonical, "iso-latin-1") {
        // Latin-1 is the identity byte→char map.
        return bytes.iter().map(|&b| b as char).collect();
    }
    let single: Option<&[(u32, u64)]> = match canonical {
        "iso-latin-2" | "iso-latin-3" | "iso-latin-4" | "iso-latin-5" | "iso-latin-9"
        | "iso-latin-10" | "us-ascii" => Some(&[]),
        "cyrillic-koi8" => Some(ENC_KOI8_R),
        "windows-1251" => Some(ENC_WINDOWS_1251),
        "mac-roman" => Some(ENC_MAC_ROMAN),
        _ => None,
    };
    if let Some(t) = single {
        return bytes
            .iter()
            .map(|&b| {
                if b < 0x80 {
                    b as char
                } else {
                    t.iter()
                        .find(|(_, p)| (p >> 56) == 1 && ((p >> 48) & 0xFF) == b as u64)
                        .and_then(|(u, _)| char::from_u32(*u))
                        .unwrap_or('?')
                }
            })
            .collect();
    }
    let multi: Option<&[(u32, u64)]> = match canonical {
        "japanese-shift-jis" | "japanese-cp932" => Some(ENC_SHIFT_JIS),
        "japanese-iso-8bit" => Some(ENC_EUC_JP),
        "chinese-big5" => Some(ENC_BIG5),
        "chinese-iso-8bit" | "chinese-gbk" => Some(ENC_GB2312),
        _ => None,
    };
    if let Some(t) = multi {
        let mut out = String::new();
        let mut ix = 0;
        while ix < bytes.len() {
            let b = bytes[ix];
            if b < 0x80 {
                out.push(b as char);
                ix += 1;
                continue;
            }
            // Try the longest sequence first (up to 4 bytes).
            let mut matched = false;
            for len in (2..=4usize).rev() {
                if ix + len > bytes.len() {
                    continue;
                }
                let seq = &bytes[ix..ix + len];
                if let Some((u, _)) = t.iter().find(|(_, p)| {
                    let n = (p >> 56) as usize;
                    n == len
                        && seq
                            .iter()
                            .enumerate()
                            .all(|(k, &bb)| ((p >> (48 - 8 * k)) & 0xFF) == bb as u64)
                }) {
                    if let Some(c) = char::from_u32(*u) {
                        out.push(c);
                        matched = true;
                    }
                    ix += len;
                    break;
                }
            }
            if !matched {
                out.push('?');
                ix += 1;
            }
        }
        return out;
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// GNU records the decoding charset of each produced char as a
/// `charset' text property.  Single-charset coding systems mark every
/// char (incl. ASCII) once any non-ASCII char exists; multi-charset
/// systems (gb2312, euc-jp, sjis, big5) mark a char with the charset
/// that decoded it, and ASCII chars inherit the "current" charset
/// (unmarked before the first non-ASCII char).  utf-8 et al. record
/// nothing.
pub(super) fn attach_decode_charset_props(i: &mut Interp, r: &StrRef, canonical: &str) {
    enum Mode {
        Single(&'static str),
        Multi(&'static [&'static str]),
    }
    let mode = match canonical {
        "iso-latin-1" => Mode::Single("iso-8859-1"),
        "iso-latin-2" => Mode::Single("iso-8859-2"),
        "iso-latin-3" => Mode::Single("iso-8859-3"),
        "iso-latin-4" => Mode::Single("iso-8859-4"),
        "iso-latin-5" => Mode::Single("iso-8859-9"),
        "iso-latin-9" => Mode::Single("iso-8859-15"),
        "iso-latin-10" => Mode::Single("iso-8859-16"),
        "cyrillic-koi8" => Mode::Single("koi8-r"),
        "windows-1251" => Mode::Single("windows-1251"),
        "mac-roman" => Mode::Single("mac-roman"),
        "japanese-shift-jis" | "japanese-cp932" => {
            Mode::Multi(&["ascii", "katakana-jisx0201", "japanese-jisx0208"])
        }
        "japanese-iso-8bit" => Mode::Multi(&[
            "ascii",
            "latin-jisx0201",
            "japanese-jisx0208",
            "katakana-jisx0201",
            "japanese-jisx0212",
            "japanese-jisx0208-1978",
        ]),
        "chinese-big5" => Mode::Multi(&["ascii", "big5"]),
        "chinese-iso-8bit" | "chinese-gbk" => Mode::Multi(&["ascii", "chinese-gb2312"]),
        _ => return,
    };
    let chars: Vec<crate::lisp::value::EmacsChar> = r.borrow().chars().collect();
    if !chars.iter().any(|&c| (c).code() as u32 >= 0x80) {
        return;
    }
    let charset_id = i.q("charset");
    let mk = |i: &mut Interp, name: &str| -> Vec<Value> {
        vec![Value::Sym(charset_id), Value::Sym(i.intern(name))]
    };
    match mode {
        Mode::Single(name) => {
            let pl = mk(i, name);
            i.set_str_props(r, vec![(0, chars.len(), pl)]);
        }
        Mode::Multi(restriction) => {
            // Runs of (start, end, charset-name); merged at the end.
            let mut runs: Vec<(usize, usize, &'static str)> = Vec::new();
            let mut cur: Option<&'static str> = None;
            for (ix, &c) in chars.iter().enumerate() {
                let u = (c).code() as u32;
                if u >= 0x80 {
                    if let Some(name) =
                        crate::lisp::builtins::strfn::char_charset_in(u as i128, restriction)
                    {
                        cur = Some(name);
                    }
                }
                if let Some(name) = cur {
                    if let Some(last) = runs.last_mut() {
                        if last.1 == ix && last.2 == name {
                            last.1 = ix + 1;
                            continue;
                        }
                    }
                    runs.push((ix, ix + 1, name));
                }
            }
            if !runs.is_empty() {
                let ivs: Vec<(usize, usize, Vec<Value>)> = runs
                    .into_iter()
                    .map(|(s, e, name)| (s, e, mk(i, name)))
                    .collect();
                i.set_str_props(r, ivs);
            }
        }
    }
}

/// File decoding keeps invalid input bytes as GNU eight-bit characters.
/// Rust's lossy UTF-8 conversion would replace them irreversibly with U+FFFD.
pub(super) fn decode_file_utf8(mut bytes: &[u8]) -> crate::lisp::text::EmacsText {
    let mut out = crate::lisp::text::EmacsText::default();
    while !bytes.is_empty() {
        match std::str::from_utf8(bytes) {
            Ok(text) => {
                out.push_str(text);
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                out.push_str(std::str::from_utf8(&bytes[..valid]).unwrap());
                let invalid = error.error_len().unwrap_or(bytes.len() - valid);
                for &byte in &bytes[valid..valid + invalid] {
                    out.push(crate::lisp::value::lisp_char(0x3FFF00 + byte as u32).unwrap());
                }
                bytes = &bytes[valid + invalid..];
            }
        }
    }
    out
}

pub(crate) fn coding_inherit_file_eol(
    i: &mut Interp,
    coding: &Value,
    parent: &Value,
) -> EvalResult {
    let coding = if coding.is_nil() {
        Value::Sym(i.q("raw-text"))
    } else {
        coding.clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    if coding_fixed_eol(i, &coding).is_some() {
        return Ok(coding);
    }
    if !parent.is_nil() {
        f_check_coding_system(i, vec![parent.clone()].into())?;
    }
    let eol = if parent.is_nil() {
        0
    } else {
        coding_fixed_eol(i, parent).unwrap_or(0)
    };
    let base = coding_canonical(i, &coding);
    Ok(Value::Sym(i.intern(&format!(
        "{}-{}",
        base,
        ["unix", "dos", "mac"][eol]
    ))))
}

pub(crate) fn raw_file_coding(i: &mut Interp, coding: &Value) -> EvalResult {
    let base = coding_canonical(i, coding);
    if matches!(base.as_str(), "no-conversion" | "raw-text") {
        return Ok(coding.clone());
    }
    let name = match coding_fixed_eol(i, coding) {
        Some(n) => format!("raw-text-{}", ["unix", "dos", "mac"][n]),
        None => "raw-text".into(),
    };
    Ok(Value::Sym(i.intern(&name)))
}

/// Decode external file bytes with the common codecs. File detection
/// keeps incomplete terminal sequences as candidates until EOF is known.
pub(crate) fn decode_file_bytes(
    i: &mut Interp,
    bytes: &[u8],
    requested: &Value,
    literal: bool,
) -> Result<(Value, Value), Flow> {
    let multibyte = i.q("enable-multibyte-characters");
    let multibyte = i.symbol_value(multibyte).truthy();
    let mut coding = if literal {
        Value::Sym(i.q("no-conversion"))
    } else if requested.is_nil() {
        Value::Sym(i.q("undecided"))
    } else if i
        .sym_id(requested)
        .is_some_and(|s| i.symbol_name(s) == "auto-save-coding")
    {
        Value::Sym(i.q("utf-8-emacs-unix"))
    } else {
        requested.clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    if !multibyte {
        coding = raw_file_coding(i, &coding)?;
    }
    if coding_canonical(i, &coding) == "undecided" {
        let fixed = coding_fixed_eol(i, &coding);
        coding = if bytes.starts_with(&[255, 254]) {
            Value::Sym(i.q("utf-16le-with-signature"))
        } else if bytes.starts_with(&[254, 255]) {
            Value::Sym(i.q("utf-16be-with-signature"))
        } else {
            crate::lisp::builtins::coding_detection::detect_file(
                i,
                &bytes.iter().map(|b| *b as i32).collect::<Vec<_>>(),
            )?
        };
        if let Some(eol) = fixed {
            if coding_canonical(i, &coding) != "no-conversion" {
                let base = coding_canonical(i, &coding);
                coding = Value::Sym(i.intern(&format!("{}-{}", base, ["unix", "dos", "mac"][eol])));
            }
        }
    }
    let source = Value::string(bytes.iter().map(|b| *b as char).collect::<String>());
    if let Value::Str(r) = &source {
        i.mark_unibyte(r);
    }
    let (decoded, used, _) = decode_region_text(i, &source, &coding)?;
    if !multibyte {
        if let Value::Str(r) = &decoded {
            i.mark_unibyte(r);
        }
    }
    Ok((decoded, used))
}

pub(crate) fn process_encode_source(i: &mut Interp, source: Value, coding: &Value) -> EvalResult {
    let hook = region_conversion_hook(i, coding, true)?;
    if hook.is_nil() {
        return Ok(source);
    }
    with_coding_buffer(i, &source, true, |i| {
        let len = crate::buffer::primitives::cur(i).borrow().text_len();
        coding_safe_call(i, &hook, vec![Value::Int(1), Value::Int(len as i128 + 1)])?;
        coding_buffer_string(i)
    })
}

pub(crate) fn process_decode_bytes(
    i: &mut Interp,
    bytes: &[u8],
    coding: &Value,
    iso: &mut crate::lisp::builtins::coding_iso2022::DecoderState,
    first: bool,
    finish: bool,
) -> Result<(Value, Value), Flow> {
    let source = Value::string(bytes.iter().map(|b| *b as char).collect::<String>());
    if let Value::Str(r) = &source {
        i.mark_unibyte(r);
    }
    let coding = if coding.is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        coding.clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    let (converted, used, raw) =
        decode_region_text_stream(i, &source, &coding, Some(iso), first, finish)?;
    Ok((coding_string_result(i, &converted, raw)?, used))
}

pub(crate) fn process_decode_result(i: &mut Interp, value: Value, coding: &Value) -> EvalResult {
    let hook = region_conversion_hook(i, coding, false)?;
    if hook.is_nil() {
        return Ok(value);
    }
    let count = want_string(i, &value)?.chars().count();
    with_coding_buffer(i, &value, false, |i| {
        region_post_read(i, &hook, 0, count)?;
        coding_buffer_string(i)
    })
}

pub(crate) fn f_encode_coding_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    code_convert_string(i, a, true)
}

pub(crate) fn f_decode_coding_string(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    code_convert_string(i, a, false)
}

/// Encode one Unicode scalar to bytes for CANONICAL (alias-group
/// head) coding-system names, mirroring GNU's `encode-coding-char'.
/// Returns None when the char is not encodable.
pub(super) fn encode_one_char(canonical: &str, c: u32) -> Option<Vec<u8>> {
    use crate::lisp::builtins::enc_tables::*;
    if c < 0x80 {
        return Some(vec![c as u8]);
    }
    let table: Option<&[(u32, u64)]> = match canonical {
        "japanese-shift-jis" | "japanese-cp932" => Some(ENC_SHIFT_JIS),
        "japanese-iso-8bit" => Some(ENC_EUC_JP),
        "cyrillic-koi8" => Some(ENC_KOI8_R),
        "windows-1251" => Some(ENC_WINDOWS_1251),
        "mac-roman" => Some(ENC_MAC_ROMAN),
        "chinese-big5" => Some(ENC_BIG5),
        "chinese-iso-8bit" | "chinese-gbk" => Some(ENC_GB2312),
        _ => None,
    };
    if let Some(t) = table {
        return t.binary_search_by_key(&c, |&(u, _)| u).ok().map(|ix| {
            let p = t[ix].1;
            let n = (p >> 56) as usize;
            p.to_be_bytes()[1..1 + n].to_vec()
        });
    }
    match canonical {
        "utf-8"
        | "mule-utf-8"
        | "cp65001"
        | "utf-8-auto"
        | "utf-8-emacs"
        | "utf-8-hfs"
        | "utf-8-nfd"
        | "utf-8-with-signature"
        | "prefer-utf-8"
        | "no-conversion"
        | "raw-text"
        | "binary"
        | "no-conversion-multibyte" => char::from_u32(c).map(|ch| ch.to_string().into_bytes()),
        "iso-latin-1" | "iso-latin-2" | "iso-latin-3" | "iso-latin-4" | "iso-latin-5"
        | "iso-latin-9" | "iso-latin-10" => {
            if c <= 0xFF {
                Some(vec![c as u8])
            } else {
                None
            }
        }
        "utf-16" | "utf-16be" | "utf-16be-with-signature" => {
            Some(vec![(c >> 8) as u8, (c & 0xFF) as u8])
        }
        "utf-16le" | "utf-16le-with-signature" => Some(vec![(c & 0xFF) as u8, (c >> 8) as u8]),
        _ => None,
    }
}

pub(super) fn f_encode_coding_char(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (encode-coding-char CHAR CODING-SYSTEM &optional CHARSET) —
    // GNU returns a unibyte string of the encoded bytes.
    let c = match &a[0] {
        Value::Int(n) if *n >= 0 && *n <= 0x3FFFFF => *n as u32,
        other => return Err(i.wrong_type_mut("characterp", other)),
    };
    let _ = f_check_coding_system(i, vec![a[1].clone()].into())?;
    let canonical = coding_canonical(i, &a[1]);
    Ok(match encode_one_char(&canonical, c) {
        Some(bs) => {
            let sv = Value::string(bs.iter().map(|&b| b as char).collect::<String>());
            if let Value::Str(r) = &sv {
                i.mark_unibyte(r);
            }
            sv
        }
        None => Value::Nil,
    })
}

/// Run Lisp conversion hooks in a disposable buffer, as coding.c does.
/// Its text properties and byte mode are part of the conversion input.
pub(super) fn with_coding_buffer(
    i: &mut Interp,
    source: &Value,
    encode: bool,
    f: impl FnOnce(&mut Interp) -> EvalResult,
) -> EvalResult {
    let old = i.current_buffer;
    let id = i.create_buffer(" *code-conversion-work*");
    i.set_current_buffer(id);
    let mb = matches!(source, Value::Str(r) if !i.is_unibyte_str(r)
        && (!encode || r.borrow().chars().any(|c| !c.is_ascii())));
    let multibyte = i.q("enable-multibyte-characters");
    i.set_buffer_local_toplevel_value(multibyte, id, if mb { Value::t() } else { Value::Nil });
    let buffer = crate::buffer::primitives::cur(i);
    buffer.borrow_mut().inhibit_buffer_hooks = true;
    if let Value::Str(r) = source {
        let storage = if mb {
            r.borrow().clone()
        } else {
            lisp_string_bytes(i, source)
                .into_iter()
                .map(|b| b as char)
                .collect()
        };
        let mut b = buffer.borrow_mut();
        let undo = b.undo_sym;
        b.locals.insert(undo, Value::t());
        b.raw_insert_at(0, &storage);
        b.text.props_write().graft(0, i.str_props(r), false);
    }
    let result = f(i);
    i.set_current_buffer(old);
    let _ = crate::buffer::primitives::kill_buffer_keep_current(i, id);
    result
}

pub(super) fn coding_buffer_string(i: &mut Interp) -> EvalResult {
    let buffer = crate::buffer::primitives::cur(i);
    let b = buffer.borrow();
    let multibyte = i.q("enable-multibyte-characters");
    let value = Value::string_with_kind(
        b.text.substring(0, b.text_len()),
        i.symbol_value(multibyte).truthy(),
    );
    if let Value::Str(r) = &value {
        i.set_str_props(r, b.text.props.borrow().slice(0, b.text_len()));
        let multibyte = i.q("enable-multibyte-characters");
        if i.symbol_value(multibyte).truthy() {
            i.mark_multibyte(r);
        } else {
            i.mark_unibyte(r);
        }
    }
    Ok(value)
}

pub(super) fn coding_safe_call(i: &mut Interp, hook: &Value, args: Vec<Value>) -> EvalResult {
    match i.apply(hook, args) {
        Err(Flow::Signal(..) | Flow::Quit) => Ok(Value::Nil),
        result => result,
    }
}

pub(crate) fn post_file_conversion(
    i: &mut Interp,
    coding: &Value,
    start: usize,
    count: usize,
) -> EvalResult {
    let hook = region_conversion_hook(i, coding, false)?;
    post_read_conversion(i, &hook, start, count, false)
}

pub(super) fn post_read_conversion(
    i: &mut Interp,
    hook: &Value,
    start: usize,
    count: usize,
    safe: bool,
) -> EvalResult {
    if hook.is_nil() {
        return Ok(Value::Int(count as i128));
    }
    let buffer = crate::buffer::primitives::cur(i);
    let before = buffer.borrow().text_len();
    let undo = buffer.borrow().undo_sym;
    let saved_undo = buffer.borrow_mut().locals.insert(undo, Value::t());
    buffer.borrow_mut().set_point(start);
    let result = if safe {
        coding_safe_call(i, hook, vec![Value::Int(count as i128)])
    } else {
        i.apply(hook, vec![Value::Int(count as i128)])
    };
    if let Some(saved) = saved_undo {
        buffer.borrow_mut().locals.insert(undo, saved);
    } else {
        buffer.borrow_mut().locals.shift_remove(&undo);
    }
    let value = result?;
    if !value.fixnump() || value.int().unwrap_or(-1) < 0 {
        return Err(i.wrong_type_mut("wholenump", &value));
    }
    Ok(Value::Int(
        count as i128 + buffer.borrow().text_len() as i128 - before as i128,
    ))
}

pub(super) fn coding_storage_text(
    i: &mut Interp,
    converted: &Value,
    encode: bool,
    raw: bool,
) -> Result<crate::lisp::text::EmacsText, Flow> {
    let text = want_string(i, converted)?;
    let unibyte_id = i.q("enable-multibyte-characters");
    let unibyte = !i.symbol_value(unibyte_id).truthy();
    Ok(if encode {
        lisp_string_bytes(i, converted)
            .into_iter()
            .map(|b| {
                if b < 128 || unibyte {
                    crate::lisp::text::EmacsChar::from(b as char)
                } else {
                    crate::lisp::value::lisp_char(0x3fff00 + b as u32).unwrap()
                }
            })
            .collect::<crate::lisp::text::EmacsText>()
    } else if unibyte {
        text.chars()
            .flat_map(|c| {
                if let Some(b) = crate::lisp::value::eight_bit_byte(c) {
                    vec![b]
                } else if raw && !c.is_ascii() {
                    vec![(-crate::lisp::value::lisp_char_code(c)) as u8]
                } else {
                    crate::lisp::builtins::coding_legacy::internal_encode(
                        crate::lisp::value::lisp_char_code(c) as u32,
                    )
                }
            })
            .map(|b| b as char)
            .collect()
    } else {
        text.clone()
    })
}

pub(super) fn coding_string_result(
    i: &mut Interp,
    converted: &Value,
    raw: bool,
) -> Result<Value, Flow> {
    if !raw {
        return Ok(converted.clone());
    }
    let text = want_string(i, converted)?;
    let value = Value::string(
        text.chars()
            .map(|c| {
                crate::lisp::value::eight_bit_byte(c).unwrap_or_else(|| {
                    if c.is_ascii() {
                        (c).code() as u8
                    } else {
                        (-crate::lisp::value::lisp_char_code(c)) as u8
                    }
                }) as char
            })
            .collect::<String>(),
    );
    if let Value::Str(r) = &value {
        i.mark_unibyte(r);
    }
    Ok(value)
}

pub(super) fn copy_coding_string(i: &mut Interp, source: &Value) -> EvalResult {
    let Value::Str(r) = source else {
        unreachable!()
    };
    let value = Value::string_with_kind(r.borrow().clone(), i.is_multibyte_str(r));
    if let Value::Str(copy) = &value {
        i.set_str_props(copy, i.str_props(r).to_vec());
        if i.is_unibyte_str(r) {
            i.mark_unibyte(copy);
        } else if i.is_multibyte_str(r) {
            i.mark_multibyte(copy);
        }
    }
    Ok(value)
}

pub(super) fn f_decode_coding_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    code_convert_region(i, a, false)
}

pub(super) fn f_encode_coding_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    code_convert_region(i, a, true)
}

pub(super) fn f_read_non_nil_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Fread_non_nil_coding_system (coding.c):
    // completing-read over `coding-system-alist' (REQUIRE-MATCH t,
    // `coding-system-history', no default) — retries while the answer is
    // empty.  Batch reads stdin; EOF propagates `end-of-file'.
    let cr = Value::Sym(i.q("completing-read"));
    let alist = {
        let id = i.q("coding-system-alist");
        if i.bound_p(id) {
            i.symbol_value(id)
        } else {
            Value::Nil
        }
    };
    let hist = Value::Sym(i.q("coding-system-history"));
    loop {
        let val = i.apply(
            &cr,
            vec![
                a[0].clone(),
                alist.clone(),
                Value::Nil,
                Value::t(),
                Value::Nil,
                hist.clone(),
                Value::Nil,
                Value::Nil,
            ],
        )?;
        match &val {
            Value::Str(s) if !s.borrow().is_empty() => {
                return crate::lisp::builtins::data::f_intern(i, vec![val.clone()].into());
            }
            _ => {}
        }
    }
}

pub(super) fn f_find_operation_coding_system(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU Ffind_operation_coding_system (coding.c): TARGET is
    // args[1 + target-idx] where `target-idx' is a plist property put on
    // the operation symbol at startup; the value comes from scanning the
    // operation's coding-system alist (file/process/network).
    if a.len() < 2 {
        return Err(i.error("Too few arguments"));
    }
    let Some(op_id) = i.sym_id(&a[0]) else {
        return Err(i.error("Invalid first argument"));
    };
    let tidx_sym = i.q("target-idx");
    let target_idx = match i.get_prop(op_id, tidx_sym) {
        Value::Int(n) if n >= 0 => n as usize,
        _ => return Err(i.error("Invalid first argument")),
    };
    if a.len() <= 1 + target_idx {
        return Err(i.error_q(
            "Too few arguments for operation `%s'",
            &[&i.symbol_name(op_id)],
        ));
    }
    let target = a[1 + target_idx].clone();
    let op_name = i.symbol_name(op_id).to_string();
    let ifc = op_name == "insert-file-contents";
    let ons = op_name == "open-network-stream";
    let target_ok = matches!(target, Value::Str(_))
        || (ifc
            && matches!(&target, Value::Cons(c)
            if matches!(&c.borrow().car, Value::Str(_))
                && matches!(&c.borrow().cdr, Value::Buffer(_))))
        || (ons
            && (matches!(target, Value::Int(_)) || matches!(target, Value::Sym(s) if s == sym::T)));
    if !target_ok {
        return Err(i.error_q(
            "Invalid argument %s of operation `%s'",
            &[&(target_idx + 1).to_string(), &op_name],
        ));
    }
    let target = if let Value::Cons(c) = target {
        c.borrow().car.clone()
    } else {
        target
    };
    let var_name = if ifc || op_name == "write-region" {
        "file-coding-system-alist"
    } else if ons {
        "network-coding-system-alist"
    } else {
        "process-coding-system-alist"
    };
    let chain = {
        let vs = i.intern(var_name);
        i.symbol_value(vs)
    };
    let mut cur = chain;
    while let Value::Cons(c) = cur.clone() {
        let b = c.borrow();
        cur = b.cdr.clone();
        let Value::Cons(elt) = &b.car else { continue };
        let (pat, val) = (elt.borrow().car.clone(), elt.borrow().cdr.clone());
        let hit = match (&pat, &target) {
            (Value::Str(pat), Value::Str(t)) => {
                // fast_string_match: case-sensitive; a bad regexp just
                // misses the element.
                let chars: Vec<crate::lisp::value::EmacsChar> = t.borrow().chars().collect();
                crate::lisp::regexp::compile_case(&pat.borrow(), false)
                    .ok()
                    .map(|re| {
                        crate::lisp::regexp::search_full(
                            &re,
                            &chars,
                            0,
                            &crate::editor::re_syntax(i),
                            &crate::editor::re_category(i),
                            crate::lisp::regexp::NO_POINT,
                            false,
                        )
                        .is_some()
                    })
                    .unwrap_or(false)
            }
            (Value::Int(p), Value::Int(t)) => p == t,
            _ => false,
        };
        if !hit {
            continue;
        }
        // val is a coding-system cons, a coding-system symbol, or a
        // function symbol called with the arg list.
        if matches!(val, Value::Cons(_)) {
            return Ok(val);
        }
        if i.sym_id(&val).is_none() {
            return Ok(Value::Nil);
        }
        if coding_known(i, &val).is_some() {
            return Ok(Value::cons(val.clone(), val));
        }
        let fid = i.sym_id(&val).unwrap();
        let f = i.symbol_function(fid);
        if matches!(f, Value::Sym(s) if s == sym::UNBOUND) {
            return Ok(Value::Nil);
        }
        let arglist = Value::list(a.to_vec());
        let val = i.apply(&f, vec![arglist])?;
        if matches!(val, Value::Cons(_)) {
            return Ok(val);
        }
        if coding_known(i, &val).is_some() {
            return Ok(Value::cons(val.clone(), val));
        }
        return Ok(Value::Nil);
    }
    Ok(Value::Nil)
}

pub(super) fn f_define_coding_system_alias(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let alias_id = want_sym(i, &a[0])?;
    let target_id = want_sym(i, &a[1])?;
    if coding_known(i, &a[1]).is_none() {
        // GNU signals `coding-system-error' with the CODING-SYSTEM arg
        // only — not (ALIAS CODING-SYSTEM).
        let cs_err = i.q("coding-system-error");
        return Err(i.signal_data(cs_err, vec![a[1].clone()]));
    }
    // GNU: `define-coding-system-alias' adds ALIAS to
    // `coding-system-list' (sorted via `add-to-coding-system-list')
    // and resolves to CODING-SYSTEM's base for `coding-system-base'.
    let alias = i.symbol_name(alias_id);
    // GNU aliases capture the target's coding object, not a live symbol
    // indirection. Freeze its canonical name (including any EOL suffix)
    // before overwriting ALIAS, so retargeting another alias cannot change
    // existing aliases and defining an alias to itself cannot create a cycle.
    let target = coding_alias_group(i, &a[1])
        .and_then(|group| group.into_iter().next())
        .unwrap_or_else(|| i.symbol_name(target_id));
    if alias == target {
        return add_to_coding_system_list(i, &alias);
    }
    if let Some((_, previous)) = i.extra_coding_aliases.iter_mut().find(|(x, _)| *x == alias) {
        *previous = target;
    } else {
        i.extra_coding_aliases.push((alias.clone(), target));
    }
    add_to_coding_system_list(i, &alias)
}

/// `composition-sort-rules` — GNU listp-checks RULES; nil.
/// `composition-get-gstring' — GNU composite.c: FONT-OBJECT decodes
/// a terminal first (`terminal-live-p' on non-nil non-font-object
/// values), then FROM/TO must be integer-or-marker; the string path
/// additionally bounds-checks via args-out-of-range.  Returns a
/// shaped lgstring vector whose lglyph arrays carry (FROM TO CHAR
/// CODE ...) tuples — enough structure for `auto-compose-chars' to
/// call compose-gstring-for-terminal without crashing.
pub(super) fn f_composition_get_gstring(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU decodes FONT-OBJECT's terminal before FROM/TO.
    if !matches!(a[2], Value::Nil) {
        let ok = match &a[2] {
            Value::Record(r) | Value::Vec(r) => {
                let r = r.borrow();
                r.first()
                    .and_then(|v| match v {
                        Value::Sym(s) => Some(i.symbol_name(*s)),
                        _ => None,
                    })
                    .map(|n| n == "font-object")
                    .unwrap_or(false)
            }
            _ => false,
        };
        if !ok {
            return Err(i.wrong_type_mut("terminal-live-p", &a[2]));
        }
    }
    let want_int = |i: &mut Interp, v: &Value| -> Result<i128, Flow> {
        match v {
            Value::Int(n) => Ok(*n),
            Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
            other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
        }
    };
    let from = want_int(i, &a[0])?;
    let to = want_int(i, &a[1])?;
    if to < from {
        return Err(crate::buffer::primitives::err_sym(
            i,
            "args-out-of-range",
            vec![a[0].clone(), a[1].clone()],
        ));
    }
    if to == from {
        return Err(crate::buffer::primitives::err_sym(
            i,
            "error",
            vec![Value::string("Attempt to shape zero-length text")],
        ));
    }
    // String path: bounds against the string length; buffer path:
    // against BEGV..ZV of the current buffer.
    let chars: Vec<u32> = match &a[3] {
        Value::Str(s) => {
            let v: Vec<u32> = s.borrow().chars().map(|c| (c).code() as u32).collect();
            if from < 0 || to as usize > v.len() || from as usize > v.len() {
                return Err(crate::buffer::primitives::err_sym(
                    i,
                    "args-out-of-range",
                    vec![a[3].clone(), Value::Int(from), Value::Int(to)],
                ));
            }
            v[from.max(0) as usize..(to.max(0) as usize).min(v.len())].to_vec()
        }
        _ => {
            let Some(b) = i.current_buffer_ref() else {
                return Err(crate::buffer::primitives::err_sym(i, "error", vec![]));
            };
            let bb = b.borrow();
            let text = bb.text.text();
            let zv = bb.text_len();
            if from < 1 || to > zv as i128 {
                return Err(crate::buffer::primitives::err_sym(
                    i,
                    "args-out-of-range",
                    vec![a[0].clone(), a[1].clone()],
                ));
            }
            text.chars()
                .skip((from - 1) as usize)
                .take((to - from) as usize)
                .map(|c| (c).code() as u32)
                .collect()
        }
    };
    // GNU lgstring layout: header [coding-system char...] then nil,
    // then one lglyph per character: [from to char code width lborder
    // rborder ascent descent adjustment ...] = [0 0 ch ch 1 0 1 1 0
    // nil] for unshaped glyphs.
    let mut lglyphs = Vec::new();
    for (_idx, ch) in chars.iter().enumerate() {
        lglyphs.push(Value::vector(vec![
            Value::Int(0),
            Value::Int(0),
            Value::Int(*ch as i128),
            Value::Int(*ch as i128),
            Value::Int(1),
            Value::Int(0),
            Value::Int(1),
            Value::Int(1),
            Value::Int(0),
            Value::Nil,
        ]));
    }
    let utf8_unix = i.q("utf-8-unix");
    let cs = Value::Sym(utf8_unix);
    let mut hdr = vec![cs];
    for ch in &chars {
        hdr.push(Value::Int(*ch as i128));
    }
    let mut v = vec![Value::vector(hdr), Value::Nil];
    v.extend(lglyphs);
    while v.len() < 10 {
        v.push(Value::Nil);
    }
    Ok(Value::vector(v))
}

pub(super) fn f_composition_sort_rules(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_list(i, &a[0])?;
    Ok(Value::Nil)
}

/// `find-composition-internal` — GNU decodes POS/LIMIT with
/// integer_or_marker checks. Static string properties are available without
/// a display composition cache; automatic/detailed compositions still need it.
pub(crate) fn f_find_composition_internal(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // (POSITION LIMIT STRING DETAIL-P)
    // GNU Ffind_composition_internal: POS via fix_position (markers
    // coerce, detached → "Marker does not point anywhere"), LIMIT
    // nil → -1 else fix_position clipped to ZV, STRING non-nil must be
    // a string and POS must lie in 0..SCHARS — else
    // `args-out-of-range (STRING POS)'; a nil STRING bounds POS to
    // BEGV..ZV with `args-out-of-range (BUFFER POS)'.
    let pos = crate::lisp::builtins::want_int_or_mark(i, &a[0])?;
    let limit = match a.get(1) {
        Some(lim) if lim.truthy() => crate::lisp::builtins::want_int_or_mark(i, lim)?,
        _ => -1,
    };
    match a.get(2) {
        Some(Value::Str(s)) => {
            let n = s.borrow().chars().count() as i128;
            if !(0..=n).contains(&pos) {
                return Err(err_sym(
                    i,
                    "args-out-of-range",
                    vec![a[2].clone(), a[0].clone()],
                ));
            }
        }
        Some(other) if other.truthy() => {
            return Err(i.wrong_type_mut("stringp", other));
        }
        _ => {
            let bb = crate::buffer::primitives::cur(i);
            let bx = bb.borrow();
            let lo = bx.begv as i128 + 1;
            let hi = bx.zv as i128 + 1;
            if !(lo..=hi).contains(&pos) {
                let buf = Value::Buffer(crate::buffer::primitives::cur(i));
                return Err(err_sym(i, "args-out-of-range", vec![buf, a[0].clone()]));
            }
        }
    }
    if let Some(Value::Str(src)) = a.get(2) {
        if !arg(&a, 3).truthy() {
            let composition = Value::Sym(i.q("composition"));
            let intervals = i.str_props(src).to_vec();
            let mut ranges: Vec<(usize, usize, Value)> = Vec::new();
            for (from, to, _) in intervals {
                let value = crate::buffer::primitives::f_get_text_property(
                    i,
                    vec![Value::Int(from as i128), composition.clone(), a[2].clone()].into(),
                )?;
                if value.is_nil() {
                    continue;
                }
                if let Some((_, end, previous)) = ranges.last_mut() {
                    if *end == from && crate::lisp::builtins::eq_values(previous, &value) {
                        *end = to;
                        continue;
                    }
                }
                ranges.push((from, to, value));
            }
            let found = ranges
                .iter()
                .find(|(from, to, _)| *from as i128 <= pos && pos < *to as i128)
                .or_else(|| {
                    if limit > pos {
                        ranges
                            .iter()
                            .find(|(from, _, _)| pos < *from as i128 && (*from as i128) < limit)
                    } else if limit >= 0 && limit < pos {
                        ranges
                            .iter()
                            .rev()
                            .find(|(_, to, _)| limit < *to as i128 && (*to as i128) <= pos)
                    } else {
                        None
                    }
                });
            if let Some((from, to, property)) = found {
                // GNU composition_valid_p accepts an unregistered
                // ((LENGTH . COMPONENTS) . MOD-FUNC) with matching length.
                let valid = if let Value::Cons(prop) = property {
                    if let Value::Cons(head) = &prop.borrow().car {
                        let head = head.borrow();
                        head.car.int() == Some((to - from) as i128)
                            && matches!(
                                head.cdr,
                                Value::Nil
                                    | Value::Int(_)
                                    | Value::Str(_)
                                    | Value::Vec(_)
                                    | Value::Cons(_)
                            )
                    } else {
                        false
                    }
                } else {
                    false
                };
                return Ok(Value::list(vec![
                    Value::Int(*from as i128),
                    Value::Int(*to as i128),
                    Value::from_bool(valid),
                ]));
            }
        }
    }
    Ok(Value::Nil)
}

/// `set-terminal-coding-system-internal` — GNU decodes TERMINAL
/// (arg 2) with terminal-live-p FIRST, then CHECK_SYMBOLs CODING,
/// then requires a known coding system (coding-system-error).
pub(super) fn f_set_terminal_coding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match arg(&a, 1) {
        Value::Nil | Value::Frame(_) => {}
        w if crate::editor::is_terminal(i, &w) => {}
        other => return Err(i.wrong_type_mut("terminal-live-p", &other)),
    }
    if i.sym_id(&a[0]).is_none() {
        return Err(i.wrong_type_mut("symbolp", &a[0]));
    }
    if !a[0].is_nil() && coding_known(i, &a[0]).is_none() {
        let s = i.q("coding-system-error");
        return Err(i.signal_data(s, vec![a[0].clone()]));
    }
    i.terminal_coding = a[0].clone();
    Ok(Value::Nil)
}

/// `set-safe-terminal-coding-system-internal` — GNU CHECK_SYMBOLs its
/// CODING-SYSTEM arg; the safe-coding result is a no-op in batch.
/// CODING-SYSTEM arg; the safe-coding result is a no-op in batch.
pub(super) fn f_set_safe_terminal_coding(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_sym(i, &a[0])?;
    Ok(Value::Nil)
}

/// `check-coding-systems-region` — GNU Fcheck_coding_systems_region:
/// START may itself be a string (checked directly → nil); otherwise
/// START/END go through CHECK_NUMBER_COERCE_MARKER, so floats and
/// other non-integers signal integer-or-marker-p.  Valid integer
/// positions are range-checked against BEGV..ZV.
pub(super) fn f_check_coding_systems_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    if matches!(a[0], Value::Str(_)) {
        return Ok(Value::Nil);
    }
    let coerce = |i: &mut Interp, v: &Value| -> Result<i128, Flow> {
        match v {
            Value::Int(n) => Ok(*n),
            Value::BigInt(_) => crate::lisp::builtins::want_int(i, v),
            Value::Marker(m) => Ok(m.borrow().position as i128 + 1),
            other => Err(i.wrong_type_mut("integer-or-marker-p", other)),
        }
    };
    let s = coerce(i, &a[0])?;
    let e = coerce(i, &a[1])?;
    let (lo, hi) = i
        .current_buffer_ref()
        .map(|b| {
            let bb = b.borrow();
            (bb.begv as i128 + 1, bb.zv as i128 + 1)
        })
        .unwrap_or((1, 1));
    if s < lo || e > hi || s > e {
        let sym = i.q("args-out-of-range");
        return Err(i.signal_data(sym, vec![a[0].clone(), a[1].clone()]));
    }
    Ok(Value::Nil)
}

/// Decode a region as external octets plus already decoded native
/// characters (GNU's negative charbuf entries). Charset annotations
/// survive native entries; ISO-2022 also resynchronizes graphic plane 0.
pub(super) fn decode_region_text(
    i: &mut Interp,
    source: &Value,
    requested: &Value,
) -> Result<(Value, Value, bool), Flow> {
    decode_region_text_stream(i, source, requested, None, true, true)
}

pub(super) fn decode_region_text_stream(
    i: &mut Interp,
    source: &Value,
    requested: &Value,
    mut iso: Option<&mut crate::lisp::builtins::coding_iso2022::DecoderState>,
    first: bool,
    finish: bool,
) -> Result<(Value, Value, bool), Flow> {
    let text = want_string(i, source)?;
    let unibyte = matches!(source, Value::Str(r) if i.is_unibyte_str(r));
    let mut bytes = Vec::new();
    let mut native = Vec::new();
    let mut storage_bytes = 0;
    for c in text.chars() {
        let raw = crate::lisp::value::eight_bit_byte(c);
        let decoded = !unibyte && raw.is_none() && (c).code() as u32 >= 128;
        bytes.push(if decoded {
            0
        } else {
            raw.unwrap_or((c).code() as u8)
        });
        native.push(decoded.then_some(c));
        storage_bytes += if unibyte || c.is_ascii() {
            1
        } else if raw.is_some() {
            2
        } else {
            crate::lisp::builtins::coding_legacy::internal_encode(
                crate::lisp::value::lisp_char_code(c) as u32,
            )
            .len()
        };
    }
    let original = coding_canonical(i, requested);
    let fixed = coding_fixed_eol(i, requested);
    let inhibit = i.q("inhibit-eol-conversion");
    let inhibit = i.symbol_value(inhibit).truthy();
    let mut canonical = original.clone();
    if original == "undecided" {
        let coding = f_detect_coding_string(i, vec![source.clone(), Value::t()].into())?;
        canonical = coding_canonical(i, &coding);
    }
    let units: Vec<i32> = bytes
        .iter()
        .enumerate()
        .map(|(p, b)| {
            native[p]
                .map(|c| -(crate::lisp::value::lisp_char_code(c) as i32))
                .unwrap_or(*b as i32)
        })
        .collect();
    canonical = crate::lisp::builtins::coding_unicode::resolve(&canonical, &units, !unibyte);
    if canonical.starts_with("utf-16") {
        let decode_name = if !first {
            canonical
                .strip_suffix("-with-signature")
                .unwrap_or(&canonical)
        } else {
            &canonical
        };
        let text = crate::lisp::builtins::coding_unicode::decode_utf16(decode_name, &units);
        let (text, detected) = crate::lisp::builtins::coding_unicode::normalize_eol(
            &text,
            if inhibit { Some(0) } else { fixed },
        );
        let eol = if inhibit { None } else { detected };
        let used = if canonical == original && (fixed.is_some() || eol.is_none()) {
            requested.clone()
        } else {
            Value::Sym(i.intern(&match eol {
                Some(n) => format!("{}-{}", canonical, ["unix", "dos", "mac"][n]),
                None => canonical,
            }))
        };
        let value = Value::string_with_kind(text, true);
        if let Value::Str(r) = &value {
            i.mark_multibyte(r);
        }
        return Ok((value, used, false));
    }
    let mut seen = [false; 3];
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'\r' if bytes.get(at + 1) == Some(&b'\n') => {
                seen[1] = true;
                at += 1;
            }
            b'\r' => seen[2] = true,
            b'\n' => seen[0] = true,
            _ => {}
        }
        at += 1;
    }
    let detected = match seen.iter().filter(|v| **v).count() {
        0 => None,
        1 => seen.iter().position(|v| *v),
        _ => Some(0),
    };
    let eol = if inhibit { None } else { fixed.or(detected) };
    let used = if canonical == original && (fixed.is_some() || eol.is_none()) {
        requested.clone()
    } else {
        let name = match eol {
            Some(n) => format!("{}-{}", canonical, ["unix", "dos", "mac"][n]),
            None => canonical.clone(),
        };
        Value::Sym(i.intern(&name))
    };
    let force = bytes.contains(&b'\r');
    if !canonical.starts_with("utf-16") {
        let mut input = Vec::new();
        let mut natives = Vec::new();
        at = 0;
        while at < bytes.len() {
            let crlf = eol == Some(1) && bytes[at..].starts_with(b"\r\n");
            input.push(if crlf || eol == Some(2) && bytes[at] == b'\r' {
                b'\n'
            } else {
                bytes[at]
            });
            natives.push(native[at]);
            at += if crlf { 2 } else { 1 };
        }
        bytes = input;
        native = natives;
    }
    let raw = matches!(canonical.as_str(), "raw-text" | "no-conversion");
    let codec = Value::Sym(i.intern(&canonical));
    let converted = if raw {
        Some((
            bytes
                .iter()
                .enumerate()
                .map(|(p, b)| {
                    native[p].unwrap_or_else(|| {
                        if *b < 128 {
                            crate::lisp::text::EmacsChar::from(*b as char)
                        } else {
                            crate::lisp::value::lisp_char(0x3fff00 + *b as u32).unwrap()
                        }
                    })
                })
                .collect(),
            Vec::new(),
        ))
    } else if let Some(result) = if let Some(state) = iso.as_deref_mut() {
        crate::lisp::builtins::coding_iso2022::decode_stream(i, &codec, &bytes, state, finish)
    } else {
        crate::lisp::builtins::coding_iso2022::decode_with_native(
            i,
            &codec,
            &bytes,
            &native,
            storage_bytes,
        )
    } {
        Some(result)
    } else if native.iter().any(Option::is_some)
        && !matches!(canonical.as_str(), "utf-8-emacs" | "utf-7" | "utf-7-imap")
    {
        crate::lisp::builtins::coding_legacy::decode_with_native(i, &codec, &bytes, &native, force)
    } else {
        None
    };
    let value = if let Some((text, spans)) = converted {
        let value = Value::string_with_kind(text, true);
        if let Value::Str(r) = &value {
            i.mark_multibyte(r);
            let charset = i.q("charset");
            let props = spans
                .into_iter()
                .map(|(s, e, name)| (s, e, vec![Value::Sym(charset), Value::Sym(i.intern(&name))]))
                .collect();
            i.set_str_props(r, props);
        }
        value
    } else {
        // Byte decoders restart at native characters; these are never
        // reinterpreted using the external coding system.
        let mut out = crate::lisp::text::EmacsText::default();
        let mut props = Vec::new();
        let mut offset = 0;
        at = 0;
        while at < bytes.len() {
            if let Some(c) = native[at] {
                out.push(c);
                offset += 1;
                at += 1;
                continue;
            }
            let start = at;
            while at < bytes.len() && native[at].is_none() {
                at += 1;
            }
            let part = &bytes[start..at];
            let segment_canonical = if (start > 0 || !first)
                && matches!(canonical.as_str(), "utf-8-auto" | "utf-8-with-signature")
            {
                "utf-8"
            } else {
                &canonical
            };
            let decoded = decode_coding_bytes(i, &codec, segment_canonical, part, force)?;
            if let Value::Str(r) = decoded {
                props.extend(
                    i.str_props(&r)
                        .iter()
                        .map(|(s, e, pl)| (s + offset, e + offset, pl.clone())),
                );
                offset += r.borrow().chars().count();
                out.push_str(&r.borrow());
            }
        }
        let value = Value::string_with_kind(out, true);
        if let Value::Str(r) = &value {
            i.mark_multibyte(r);
            i.set_str_props(r, props);
        }
        value
    };
    Ok((value, used, raw))
}

pub(super) fn region_conversion_hook(
    i: &mut Interp,
    coding: &Value,
    encode: bool,
) -> Result<Value, Flow> {
    let plist = f_coding_system_plist(i, vec![coding.clone()].into())?;
    let property = i.intern(if encode {
        ":pre-write-conversion"
    } else {
        ":post-read-conversion"
    });
    let hook = crate::lisp::eval::plist_get(&plist, property);
    // These Lisp hooks are already implemented by the shared byte codecs.
    let implemented = i.sym_id(&hook).is_some_and(|s| {
        matches!(
            i.symbol_name(s).as_str(),
            "utf-7-pre-write-conversion"
                | "utf-7-post-read-conversion"
                | "utf-7-imap-pre-write-conversion"
                | "utf-7-imap-post-read-conversion"
                | "post-read-decode-hz"
                | "pre-write-encode-hz"
        )
    });
    Ok(if implemented { Value::Nil } else { hook })
}

pub(super) fn region_post_read(
    i: &mut Interp,
    hook: &Value,
    start: usize,
    count: usize,
) -> EvalResult {
    post_read_conversion(i, hook, start, count, true)
}

pub(super) fn code_convert_region(
    i: &mut Interp,
    a: crate::lisp::Argv,
    encode: bool,
) -> EvalResult {
    let deactivate = i.q("deactivate-mark");
    let saved = i.symbol_value(deactivate);
    let result = code_convert_region_inner(i, a, encode);
    let _ = i.set_symbol(deactivate, saved);
    result
}

pub(super) fn code_convert_region_inner(
    i: &mut Interp,
    a: crate::lisp::Argv,
    encode: bool,
) -> EvalResult {
    use crate::buffer::primitives::{chg_coding_region, chg_with_buffer};
    let coding = if a[2].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[2].clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    let destination = arg(&a, 3);
    let string_result = matches!(destination, Value::Sym(sym::T));
    let source_buffer = i.current_buffer;
    let destination_buffer = match &destination {
        Value::Nil => source_buffer,
        Value::Sym(sym::T) => source_buffer,
        Value::Buffer(b) => b.borrow().id,
        v => return Err(i.wrong_type_mut("bufferp", v)),
    };
    let (positions, mut source) = coding_region_source(i, &a)?;
    let hook = region_conversion_hook(i, &coding, encode)?;
    if encode && !hook.is_nil() {
        source = with_coding_buffer(i, &source, true, |i| {
            let len = crate::buffer::primitives::cur(i).borrow().text_len();
            coding_safe_call(i, &hook, vec![Value::Int(1), Value::Int(len as i128 + 1)])?;
            coding_buffer_string(i)
        })?;
    }
    let (converted, used, raw) = if encode {
        (
            encode_coding_text(i, vec![source.clone(), coding.clone()].into())?,
            coding,
            false,
        )
    } else {
        decode_region_text(i, &source, &coding)?
    };
    let text = want_string(i, &converted)?;
    let count = text.chars().count();
    let mut result;
    if string_result {
        result = coding_string_result(i, &converted, raw)?;
        if !encode && !hook.is_nil() {
            result = with_coding_buffer(i, &result, false, |i| {
                region_post_read(i, &hook, 0, count)?;
                coding_buffer_string(i)
            })?;
        }
    } else {
        let same = source_buffer == destination_buffer;
        result = chg_with_buffer(i, destination_buffer, |i| {
            let storage = coding_storage_text(i, &converted, encode, raw)?;
            let b = crate::buffer::primitives::cur(i);
            let (start, end) = if same {
                (positions[0] as usize - 1, positions[1] as usize - 1)
            } else {
                let point = b.borrow().point();
                (point, point)
            };
            chg_coding_region(i, start, end, &storage, &converted, same, encode, |i| {
                if encode {
                    Ok(Value::Int(count as i128))
                } else {
                    region_post_read(i, &hook, start, count)
                }
            })
        })?;
    }
    let last = i.q("last-coding-system-used");
    i.set_symbol(last, used)?;
    Ok(result)
}

pub(super) fn code_convert_string(
    i: &mut Interp,
    a: crate::lisp::Argv,
    encode: bool,
) -> EvalResult {
    let text = want_string(i, &a[0])?;
    let destination = arg(&a, 3);
    let last = i.q("last-coding-system-used");
    if a[1].is_nil() && destination.is_nil() {
        let no_conversion = Value::Sym(i.q("no-conversion"));
        i.set_symbol(last, no_conversion)?;
        return if arg(&a, 2).truthy() {
            Ok(a[0].clone())
        } else {
            copy_coding_string(i, &a[0])
        };
    }
    let coding = if a[1].is_nil() {
        Value::Sym(i.q("no-conversion"))
    } else {
        a[1].clone()
    };
    f_check_coding_system(i, vec![coding.clone()].into())?;
    let target = match &destination {
        Value::Nil | Value::Sym(sym::T) => None,
        Value::Buffer(b) => Some(b.borrow().id),
        v => return Err(i.wrong_type_mut("bufferp", v)),
    };
    let plist = f_coding_system_plist(i, vec![coding.clone()].into())?;
    let ascii_key = i.q(":ascii-compatible-p");
    let inhibit = i.q("inhibit-eol-conversion");
    if target.is_none()
        && text.is_ascii()
        && crate::lisp::eval::plist_get(&plist, ascii_key).truthy()
        && (coding_fixed_eol(i, &coding) == Some(0)
            || i.symbol_value(inhibit).truthy()
            || !text.contains(if encode { '\n' } else { '\r' }))
    {
        i.set_symbol(last, coding)?;
        if arg(&a, 2).truthy() {
            return Ok(a[0].clone());
        }
        let value = Value::string_with_kind(text, !encode);
        if let Value::Str(r) = &value {
            if encode {
                i.mark_unibyte(r);
            } else {
                i.mark_multibyte(r);
            }
        }
        return Ok(value);
    }
    let hook = region_conversion_hook(i, &coding, encode)?;
    let mut source = a[0].clone();
    if encode && !hook.is_nil() {
        source = with_coding_buffer(i, &source, true, |i| {
            let len = crate::buffer::primitives::cur(i).borrow().text_len();
            coding_safe_call(i, &hook, vec![Value::Int(1), Value::Int(len as i128 + 1)])?;
            coding_buffer_string(i)
        })?;
    }
    let (converted, used, raw) = if encode {
        (
            encode_coding_text(i, vec![source, coding.clone()].into())?,
            coding,
            false,
        )
    } else {
        decode_region_text(i, &source, &coding)?
    };
    let count = want_string(i, &converted)?.chars().count();
    let result = if let Some(target) = target {
        crate::buffer::primitives::chg_with_buffer(i, target, |i| {
            let storage = coding_storage_text(i, &converted, encode, raw)?;
            let point = crate::buffer::primitives::cur(i).borrow().point();
            crate::buffer::primitives::chg_coding_region(
                i,
                point,
                point,
                &storage,
                &converted,
                false,
                encode,
                |i| {
                    if encode {
                        Ok(Value::Int(count as i128))
                    } else {
                        region_post_read(i, &hook, point, count)
                    }
                },
            )
        })?
    } else {
        let result = coding_string_result(i, &converted, raw)?;
        if !encode && !hook.is_nil() {
            with_coding_buffer(i, &result, false, |i| {
                region_post_read(i, &hook, 0, count)?;
                coding_buffer_string(i)
            })?
        } else {
            result
        }
    };
    i.set_symbol(last, used)?;
    Ok(result)
}

#[cfg(test)]
mod wide_file_utf8_tests {
    use super::*;
    #[test]
    fn private_use_and_invalid_octets_stay_distinct() {
        let decoded = decode_file_utf8(&[0xf3, 0xb0, 0x82, 0x80, 0x80, 0xff, 65]);
        assert_eq!(
            decoded.chars().map(|c| c.code()).collect::<Vec<_>>(),
            vec![0xf0080, 0x3fff80, 0x3fffff, 65]
        );
    }
    #[test]
    fn truncated_sequences_preserve_every_octet() {
        let decoded = decode_file_utf8(&[65, 0xe3, 0x81]);
        assert_eq!(
            decoded.chars().map(|c| c.code()).collect::<Vec<_>>(),
            vec![65, 0x3fffe3, 0x3fff81]
        );
    }
}
