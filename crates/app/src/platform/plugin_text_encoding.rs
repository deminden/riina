//! Native decoders for legacy plugin encodings outside the common web encodings.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[derive(Debug)]
pub(crate) enum DecodeError {
    Unsupported,
    Invalid,
}

macro_rules! charsets {
    ($($name:literal => $apple:ident, $unix:expr, $windows:expr);+ $(;)?) => {
        pub(crate) fn decode(name: &str, bytes: &[u8]) -> Option<Result<String, DecodeError>> {
            match name {
                $(
                    $name => {
                        #[cfg(target_os = "macos")]
                        let result = macos::decode(bytes, macos::Encoding::$apple);
                        #[cfg(target_os = "linux")]
                        let result = linux::decode(bytes, $unix);
                        #[cfg(windows)]
                        let result = windows::decode(bytes, $windows);
                        Some(result)
                    }
                )+
                _ => None,
            }
        }
    };
}

charsets! {
    "ansel" => ANSEL, None, None;
    "cns-11643-92-p1" => CNS_11643_92_P1, None, None;
    "cns-11643-92-p2" => CNS_11643_92_P2, None, None;
    "cns-11643-92-p3" => CNS_11643_92_P3, None, None;
    "dosarabic" => DOSArabic, Some(c"CP864"), Some(864);
    "dosbalticrim" => DOSBalticRim, Some(c"CP775"), Some(775);
    "doscanadianfrench" => DOSCanadianFrench, Some(c"CP863"), Some(863);
    "doschinesesimplif" => DOSChineseSimplif, Some(c"CP936"), Some(936);
    "doschinesetrad" => DOSChineseTrad, Some(c"CP950"), Some(950);
    "doscyrillic" => DOSCyrillic, Some(c"CP855"), Some(855);
    "dosgreek" => DOSGreek, Some(c"CP737"), Some(737);
    "dosgreek1" => DOSGreek1, Some(c"CP851"), None;
    "dosgreek2" => DOSGreek2, Some(c"CP869"), Some(869);
    "doshebrew" => DOSHebrew, Some(c"CP862"), Some(862);
    "dosicelandic" => DOSIcelandic, Some(c"CP861"), Some(861);
    "dosjapanese" => DOSJapanese, Some(c"CP932"), Some(932);
    "doskorean" => DOSKorean, Some(c"CP949"), Some(949);
    "doslatin1" => DOSLatin1, Some(c"CP850"), Some(850);
    "doslatin2" => DOSLatin2, Some(c"CP852"), Some(852);
    "doslatinus" => DOSLatinUS, Some(c"CP437"), Some(437);
    "dosnordic" => DOSNordic, Some(c"CP865"), Some(865);
    "dosportuguese" => DOSPortuguese, Some(c"CP860"), Some(860);
    "dosrussian" => DOSRussian, Some(c"CP866"), Some(866);
    "dosthai" => DOSThai, Some(c"CP874"), Some(874);
    "dosturkish" => DOSTurkish, Some(c"CP857"), Some(857);
    "ebcdic-cp037" => EBCDIC_CP037, Some(c"IBM037"), Some(37);
    "ebcdic-us" => EBCDIC_US, None, None;
    "euc-tw" => EUC_TW, Some(c"EUC-TW"), Some(51950);
    "hz-gb-2312" => HZ_GB_2312, None, None;
    "iso-2022-cn" => ISO_2022_CN, Some(c"ISO-2022-CN"), None;
    "iso-2022-cn-ext" => ISO_2022_CN_EXT, Some(c"ISO-2022-CN-EXT"), None;
    "iso-2022-kr" => ISO_2022_KR, Some(c"ISO-2022-KR"), None;
    "jis-c6226-78" => JIS_C6226_78, None, None;
    "jis-x0201-76" => JIS_X0201_76, None, None;
    "jis-x0208-83" => JIS_X0208_83, None, None;
    "jis-x0208-90" => JIS_X0208_90, None, None;
    "jis-x0212-90" => JIS_X0212_90, None, None;
    "ksc-5601-87" => KSC_5601_87, None, None;
    "ksc-5601-92-johab" => KSC_5601_92_Johab, Some(c"JOHAB"), Some(1361);
    "mackorean" => MacKorean, None, Some(10003);
    "macarabic" => MacArabic, None, Some(10004);
    "macarmenian" => MacArmenian, None, None;
    "macbengali" => MacBengali, None, None;
    "macburmese" => MacBurmese, None, None;
    "macceltic" => MacCeltic, None, None;
    "macchinesesimp" => MacChineseSimp, None, Some(10008);
    "macchinesetrad" => MacChineseTrad, None, Some(10002);
    "macdevanagari" => MacDevanagari, None, None;
    "macethiopic" => MacEthiopic, None, None;
    "macextarabic" => MacExtArabic, None, None;
    "macfarsi" => MacFarsi, None, None;
    "macgaelic" => MacGaelic, None, None;
    "macgeorgian" => MacGeorgian, None, None;
    "macgujarati" => MacGujarati, None, None;
    "macgurmukhi" => MacGurmukhi, None, None;
    "machfs" => MacHFS, None, None;
    "machebrew" => MacHebrew, None, Some(10005);
    "macgreek" => MacGreek, Some(c"MAC-GREEK"), Some(10006);
    "maccyrillic" => MacCyrillic, Some(c"MAC-CYRILLIC"), Some(10007);
    "maccentraleurroman" => MacCentralEurRoman, Some(c"MAC-CENTRALEUROPE"), Some(10029);
    "macturkish" => MacTurkish, None, Some(10081);
    "maccroatian" => MacCroatian, None, Some(10082);
    "macicelandic" => MacIcelandic, Some(c"MAC-IS"), Some(10079);
    "macinuit" => MacInuit, None, None;
    "macjapanese" => MacJapanese, None, Some(10001);
    "mackannada" => MacKannada, None, None;
    "mackhmer" => MacKhmer, None, None;
    "maclaotian" => MacLaotian, None, None;
    "macmalayalam" => MacMalayalam, None, None;
    "macmongolian" => MacMongolian, None, None;
    "macoriya" => MacOriya, None, None;
    "macromanlatin1" => MacRomanLatin1, None, None;
    "macromanian" => MacRomanian, None, Some(10010);
    "macsinhalese" => MacSinhalese, None, None;
    "macsymbol" => MacSymbol, None, None;
    "symbol" => MacSymbol, None, None;
    "macdingbats" => MacDingbats, None, None;
    "mactamil" => MacTamil, None, None;
    "mactelugu" => MacTelugu, None, None;
    "macthai" => MacThai, None, Some(10021);
    "mactibetan" => MacTibetan, None, None;
    "macukrainian" => MacUkrainian, Some(c"MAC-UK"), Some(10017);
    "macvietnamese" => MacVietnamese, None, None;
    "macvt100" => MacVT100, None, None;
    "nextstepjapanese" => NextStepJapanese, None, None;
    "shiftjis-x0213" => ShiftJIS_X0213, Some(c"SHIFT_JISX0213"), None;
    "shiftjis-x0213-menkuten" => ShiftJIS_X0213_MenKuTen, None, None;
    "utf7" => UTF7, Some(c"UTF-7"), None;
    "utf7-imap" => UTF7_IMAP, Some(c"UTF-7-IMAP"), None;
    "viscii" => VISCII, Some(c"VISCII"), None;
    "windowskoreanjohab" => WindowsKoreanJohab, Some(c"JOHAB"), Some(1361);
}
